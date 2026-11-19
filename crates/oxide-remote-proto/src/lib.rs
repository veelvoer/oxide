//! The Oxide Remote protocol, shared by the PC side (`oxide-remoted`) and the phone app.
//!
//! * **Discovery**: the phone sends [`DISCOVER`] as a UDP broadcast to port [`DISCOVERY_PORT`]; every PC answers
//!   with a JSON [`Announce`] (name, id, TCP port, MAC addresses for Wake-on-LAN).
//! * **Pairing** (once per phone and PC): the PC shows a 6-digit code. Both sides do an X25519 key exchange; the
//!   code is mixed into the key derivation, and each side proves it knows the code with an HMAC over the
//!   exchange. Only then a long-term key is stored on both sides. Someone in the middle would have to guess the
//!   code in a single try.
//! * **Session**: a fresh exchange of random nonces derives one ChaCha20-Poly1305 key per direction from the
//!   long-term key. Every later frame is encrypted and authenticated; a wrong key fails at the first frame.
//! * **Framing**: `u32 length (big endian)`, then the (encrypted) body. A body starts with a kind byte:
//!   [`KIND_JSON`] (a [`ToPc`] / [`ToPhone`] message) or [`KIND_FRAME`] (a JPEG screen frame with a small header).
pub mod watch;

use std::io::{Read, Write};

use chacha20poly1305::{aead::Aead, ChaCha20Poly1305, KeyInit, Nonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

pub const TCP_PORT: u16 = 47820;
pub const DISCOVERY_PORT: u16 = 47821;
pub const DISCOVER: &[u8] = b"OXIDE-REMOTE-DISCOVER-1";
pub const KIND_JSON: u8 = 1;
pub const KIND_FRAME: u8 = 2;
/// Frames larger than this are refused (a 4K JPEG is far below).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// A PC answering discovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Announce {
    pub id: String,
    pub name: String,
    pub port: u16,
    /// MAC addresses of the PC's wired/wireless interfaces, for Wake-on-LAN later
    pub macs: Vec<String>,
    pub version: u32,
}

/// Plain messages before the session is encrypted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Hello {
    /// phone → PC: start pairing (the PC must be in pairing mode: "Pair a phone" in its settings)
    PairHello { phone_id: String, phone_name: String, epk: String },
    /// PC → phone: its half of the exchange; the user now types the code shown on the PC
    PairChallenge { pc_id: String, pc_name: String, epk: String, macs: Vec<String> },
    /// phone → PC: proof of the code
    PairProof { mac: String },
    /// PC → phone: paired; the PC's proof of the code
    PairOk { mac: String },
    /// phone → PC: open a session (already paired)
    Hello { phone_id: String, nonce: String },
    /// PC → phone: its nonce; the next frames are encrypted
    HelloOk { nonce: String, pc_name: String, pc_id: String },
    /// either way: refused, with a reason a person can read
    Refused { reason: String },
}

/// Requests from the phone (encrypted).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToPc {
    Ping,
    Info,
    Stats,
    /// start sending screen frames: at most `max_w` pixels wide, JPEG quality 1-100, at most `fps`
    ScreenStart { max_w: u32, quality: u8, fps: u8, output: Option<String> },
    ScreenStop,
    /// the phone has shown frame `seq` (the PC keeps at most two frames in flight: low delay, no backlog)
    FrameAck { seq: u64 },
    /// pointer in screen fractions (0..1) of the shown output
    PointerTo { x: f64, y: f64 },
    PointerBy { dx: f64, dy: f64 },
    Click { button: String },
    ButtonState { button: String, pressed: bool },
    Scroll { dx: f64, dy: f64 },
    Key { name: String, mods: Vec<String> },
    Text { text: String },
    /// lock, suspend, reboot, shutdown, logout, media_play_pause, media_next, media_prev, volume (arg 0-100),
    /// mute, workspace (arg number), run (arg command), open_url (arg url), notify (arg text), clipboard_set (arg text)
    Action { name: String, arg: Option<String> },
    ClipboardGet,
    /// Say something to the PC's assistant (it works on the PC and answers with `ToPhone::Assistant` events).
    Ask { text: String },
    /// Answer to an assistant `confirm` event.
    AskConfirm { id: u64, allow: bool },
    AskCancel,
    /// Forget the conversation.
    AskReset,
}

/// Answers and events from the PC (encrypted).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ToPhone {
    Pong,
    Info(PcInfo),
    Stats(Stats),
    Done { ok: bool, message: String },
    Clipboard { text: String },
    /// Where the PC can be reached away from home (the phone keeps these for later).
    Whereabouts(Whereabouts),
    /// One event of the assistant (`oxide-ai`'s JSON line as is: text_delta, tool_start, confirm, done, error, ...).
    Assistant { json: String },
}

/// The PC's addresses on the internet and the ntfy server where it leaves an encrypted note when they change.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Whereabouts {
    /// "host:port" or "[v6]:port"
    pub addrs: Vec<String>,
    /// the PC's addresses on the network it is on now (for when the phone is on that network too)
    pub lan: Vec<String>,
    pub rendezvous: String,
    pub at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PcInfo {
    pub name: String,
    pub hostname: String,
    pub os: String,
    pub macs: Vec<String>,
    pub outputs: Vec<String>,
    /// Oxide is running (input control, workspaces)
    pub oxide: bool,
    pub can_capture: bool,
    /// the PC's Oxide colors (hex): background, card, text, muted, accent, text on accent
    #[serde(default)]
    pub palette: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Stats {
    pub cpu: f32,
    pub cores: Vec<f32>,
    pub load: [f32; 3],
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub disks: Vec<Disk>,
    pub temps: Vec<Temp>,
    pub net_rx: u64,
    pub net_tx: u64,
    pub battery: Option<f32>,
    pub charging: bool,
    pub uptime: u64,
    pub top: Vec<Proc>,
    pub gpu: Option<f32>,
    pub volume: Option<f32>,
    pub muted: bool,
    pub media: Option<String>,
    pub workspace: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Disk {
    pub mount: String,
    pub used: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Temp {
    pub label: String,
    pub celsius: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Proc {
    pub name: String,
    pub cpu: f32,
    pub mem: u64,
}

/// A screen frame: sequence number, size of the full output, and the JPEG.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub seq: u64,
    pub width: u32,
    pub height: u32,
    pub jpeg: Vec<u8>,
}

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.jpeg.len() + 17);
        v.push(KIND_FRAME);
        v.extend_from_slice(&self.seq.to_be_bytes());
        v.extend_from_slice(&self.width.to_be_bytes());
        v.extend_from_slice(&self.height.to_be_bytes());
        v.extend_from_slice(&self.jpeg);
        v
    }
    pub fn decode(b: &[u8]) -> Option<Frame> {
        if b.len() < 17 || b[0] != KIND_FRAME {
            return None;
        }
        Some(Frame {
            seq: u64::from_be_bytes(b[1..9].try_into().ok()?),
            width: u32::from_be_bytes(b[9..13].try_into().ok()?),
            height: u32::from_be_bytes(b[13..17].try_into().ok()?),
            jpeg: b[17..].to_vec(),
        })
    }
}

pub fn json_body<T: Serialize>(m: &T) -> Vec<u8> {
    let mut v = vec![KIND_JSON];
    v.extend_from_slice(&serde_json::to_vec(m).unwrap_or_default());
    v
}

// ---------------------------------------------------------------- framing

pub fn write_frame(w: &mut impl Write, body: &[u8]) -> std::io::Result<()> {
    w.write_all(&(body.len() as u32).to_be_bytes())?;
    w.write_all(body)?;
    w.flush()
}

pub fn read_frame(r: &mut impl Read) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut v = vec![0u8; n];
    r.read_exact(&mut v)?;
    Ok(v)
}

pub fn write_hello(w: &mut impl Write, m: &Hello) -> std::io::Result<()> {
    write_frame(w, &serde_json::to_vec(m).unwrap_or_default())
}

pub fn read_hello(r: &mut impl Read) -> std::io::Result<Hello> {
    let b = read_frame(r)?;
    serde_json::from_slice(&b).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

// ---------------------------------------------------------------- crypto

pub fn random<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).expect("system random");
    b
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// One side of a pairing: its ephemeral key.
pub struct PairKey {
    secret: x25519_dalek::StaticSecret,
    pub public: [u8; 32],
}

impl PairKey {
    pub fn new() -> Self {
        let secret = x25519_dalek::StaticSecret::from(random::<32>());
        let public = x25519_dalek::PublicKey::from(&secret).to_bytes();
        Self { secret, public }
    }
    pub fn shared(&self, peer: &[u8; 32]) -> [u8; 32] {
        self.secret.diffie_hellman(&x25519_dalek::PublicKey::from(*peer)).to_bytes()
    }
}

impl Default for PairKey {
    fn default() -> Self {
        Self::new()
    }
}

/// Keys that follow from a pairing: (confirmation key, long-term key). `phone_epk`/`pc_epk` bind the result to
/// this exchange, the code makes it worthless to anyone who does not know the code.
pub fn pair_keys(shared: &[u8; 32], code: &str, phone_epk: &[u8; 32], pc_epk: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let mut transcript = Vec::with_capacity(64);
    transcript.extend_from_slice(phone_epk);
    transcript.extend_from_slice(pc_epk);
    let hk = Hkdf::<Sha256>::new(Some(code.as_bytes()), shared);
    let mut confirm = [0u8; 32];
    let mut ltk = [0u8; 32];
    let _ = hk.expand(&[b"oxide-remote pair confirm".as_slice(), &transcript].concat(), &mut confirm);
    let _ = hk.expand(&[b"oxide-remote long-term key".as_slice(), &transcript].concat(), &mut ltk);
    (confirm, ltk)
}

pub fn proof(confirm: &[u8; 32], who: &str) -> String {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(confirm).expect("hmac key");
    m.update(who.as_bytes());
    hex(&m.finalize().into_bytes())
}

pub fn check_proof(confirm: &[u8; 32], who: &str, mac_hex: &str) -> bool {
    let Some(mac) = unhex(mac_hex) else { return false };
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(confirm).expect("hmac key");
    m.update(who.as_bytes());
    m.verify_slice(&mac).is_ok()
}

/// One direction of an encrypted session.
pub struct Cipher {
    aead: ChaCha20Poly1305,
    counter: u64,
}

impl Cipher {
    fn nonce(&mut self) -> Nonce {
        let mut n = [0u8; 12];
        n[4..].copy_from_slice(&self.counter.to_be_bytes());
        self.counter += 1;
        *Nonce::from_slice(&n)
    }
    pub fn seal(&mut self, plain: &[u8]) -> Vec<u8> {
        let n = self.nonce();
        self.aead.encrypt(&n, plain).unwrap_or_default()
    }
    pub fn open(&mut self, sealed: &[u8]) -> Option<Vec<u8>> {
        let n = self.nonce();
        self.aead.decrypt(&n, sealed).ok()
    }
}

/// Session ciphers for (phone → PC, PC → phone).
pub fn session(ltk: &[u8; 32], phone_nonce: &[u8], pc_nonce: &[u8]) -> (Cipher, Cipher) {
    let hk = Hkdf::<Sha256>::new(Some(&[phone_nonce, pc_nonce].concat()), ltk);
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    let _ = hk.expand(b"oxide-remote phone->pc", &mut a);
    let _ = hk.expand(b"oxide-remote pc->phone", &mut b);
    (
        Cipher { aead: ChaCha20Poly1305::new_from_slice(&a).expect("key"), counter: 0 },
        Cipher { aead: ChaCha20Poly1305::new_from_slice(&b).expect("key"), counter: 0 },
    )
}

// ------------------------------------------------------------- notes on a rendezvous server
//
// When the PC's address changes it leaves a note for each paired phone on a public pub/sub server (ntfy). Only
// that phone can find the topic (derived from the pairing key) and read the note (sealed with a key derived from it).

fn note_key(ltk: &[u8; 32], what: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(b"oxide-remote note"), ltk);
    let mut k = [0u8; 32];
    let _ = hk.expand(what, &mut k);
    k
}

/// The topic name for one pairing.
pub fn note_topic(ltk: &[u8; 32]) -> String {
    format!("oxr{}", &hex(&note_key(ltk, b"topic"))[..40])
}

pub fn note_seal(ltk: &[u8; 32], plain: &[u8]) -> String {
    let aead = ChaCha20Poly1305::new_from_slice(&note_key(ltk, b"seal")).expect("key");
    let n = random::<12>();
    let ct = aead.encrypt(Nonce::from_slice(&n), plain).unwrap_or_default();
    hex(&[&n[..], &ct].concat())
}

pub fn note_open(ltk: &[u8; 32], note: &str) -> Option<Vec<u8>> {
    let b = unhex(note.trim())?;
    if b.len() < 13 {
        return None;
    }
    let aead = ChaCha20Poly1305::new_from_slice(&note_key(ltk, b"seal")).ok()?;
    aead.decrypt(Nonce::from_slice(&b[..12]), &b[12..]).ok()
}

/// Whether an address belongs to the local network (pairing is only allowed from there).
pub fn is_local(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v) => v.is_private() || v.is_loopback() || v.is_link_local() || (v.octets()[0] == 100 && (v.octets()[1] & 0xc0) == 64),
        std::net::IpAddr::V6(v) => {
            if let Some(m) = v.to_ipv4_mapped() {
                return is_local(std::net::IpAddr::V4(m));
            }
            v.is_loopback() || (v.segments()[0] & 0xfe00) == 0xfc00 || (v.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// A Wake-on-LAN magic packet for `mac` ("aa:bb:cc:dd:ee:ff").
pub fn magic_packet(mac: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = mac.split([':', '-']).map(|p| u8::from_str_radix(p, 16).ok()).collect::<Option<Vec<u8>>>()?;
    if bytes.len() != 6 {
        return None;
    }
    let mut v = vec![0xffu8; 6];
    for _ in 0..16 {
        v.extend_from_slice(&bytes);
    }
    Some(v)
}

/// Send magic packets for `mac` to the usual broadcast addresses (and `subnet_broadcast` when known).
pub fn wake(mac: &str, subnet_broadcast: Option<std::net::Ipv4Addr>) -> std::io::Result<()> {
    let pkt = magic_packet(mac).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad MAC address"))?;
    let sock = std::net::UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    let mut targets = vec![std::net::Ipv4Addr::BROADCAST];
    if let Some(b) = subnet_broadcast {
        targets.push(b);
    }
    for t in targets {
        for port in [9u16, 7] {
            let _ = sock.send_to(&pkt, (t, port));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_roundtrip() {
        let k = [7u8; 32];
        let n = note_seal(&k, b"hello");
        assert_eq!(note_open(&k, &n).as_deref(), Some(&b"hello"[..]));
        assert!(note_open(&[8u8; 32], &n).is_none());
        assert!(note_topic(&k).starts_with("oxr") && note_topic(&k).len() == 43);
    }

    #[test]
    fn pairing_agrees_only_with_the_right_code() {
        let (phone, pc) = (PairKey::new(), PairKey::new());
        let s1 = phone.shared(&pc.public);
        let s2 = pc.shared(&phone.public);
        assert_eq!(s1, s2);
        let (c1, l1) = pair_keys(&s1, "123456", &phone.public, &pc.public);
        let (c2, l2) = pair_keys(&s2, "123456", &phone.public, &pc.public);
        assert_eq!((c1, l1), (c2, l2));
        assert!(check_proof(&c2, "phone", &proof(&c1, "phone")));
        let (c3, _) = pair_keys(&s1, "654321", &phone.public, &pc.public);
        assert!(!check_proof(&c2, "phone", &proof(&c3, "phone")));
    }

    #[test]
    fn session_round_trip_and_tamper() {
        let ltk = random::<32>();
        let (np, nc) = (random::<16>(), random::<16>());
        let (mut a_out, mut a_in) = session(&ltk, &np, &nc);
        let (mut b_in, mut b_out) = session(&ltk, &np, &nc);
        let sealed = a_out.seal(b"hello pc");
        assert_eq!(b_in.open(&sealed).unwrap(), b"hello pc");
        let back = b_out.seal(b"hello phone");
        assert_eq!(a_in.open(&back).unwrap(), b"hello phone");
        let mut bad = a_out.seal(b"x");
        bad[0] ^= 1;
        assert!(b_in.open(&bad).is_none());
    }

    #[test]
    fn frames_and_wol() {
        let f = Frame { seq: 7, width: 1920, height: 1080, jpeg: vec![1, 2, 3] };
        assert_eq!(Frame::decode(&f.encode()), Some(f));
        let p = magic_packet("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(p.len(), 102);
        assert_eq!(&p[6..12], &[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        assert!(magic_packet("nope").is_none());
    }
}
