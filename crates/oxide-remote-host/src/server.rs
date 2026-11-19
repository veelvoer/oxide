//! The network side: discovery (UDP), pairing and encrypted sessions (TCP), screen streaming.
use std::{
    io::Write,
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use oxide_remote_proto::*;

use crate::{
    platform::{self as plat, Capturer},
    stats::Sampler,
    store::{self, Phone, Store},
};

/// Pairing mode: the code and until when it is valid, and the failed tries.
#[derive(Default)]
pub struct Pairing {
    pub code: Option<(String, Instant)>,
    pub failures: u32,
}

#[derive(Clone)]
pub struct Shared {
    pub store: Arc<Mutex<Store>>,
    pub pairing: Arc<Mutex<Pairing>>,
    pub sampler: Sampler,
    pub reach: Arc<Mutex<crate::reach::Reach>>,
    /// text for the window: what happened last (a phone connected, …)
    pub log: Arc<Mutex<Vec<String>>>,
    /// connections from outside the local network that are still in the handshake
    pub remote_handshakes: Arc<std::sync::atomic::AtomicUsize>,
}

/// The ports (the normal ones; OXIDE_REMOTE_HOST_PORT changes them for testing next to another daemon).
pub fn tcp_port() -> u16 {
    std::env::var("OXIDE_REMOTE_HOST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(TCP_PORT)
}
pub fn discovery_port() -> u16 {
    std::env::var("OXIDE_REMOTE_HOST_DISCOVERY").ok().and_then(|p| p.parse().ok()).unwrap_or(DISCOVERY_PORT)
}

pub fn discovery(shared: Shared) {
    std::thread::spawn(move || {
        let sock = match UdpSocket::bind(("0.0.0.0", discovery_port())) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("oxide-remoted: discovery port {DISCOVERY_PORT} unavailable: {e}");
                return;
            }
        };
        let mut buf = [0u8; 256];
        loop {
            let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
            if &buf[..n] != DISCOVER {
                continue;
            }
            let s = shared.store.lock().map(|s| s.clone()).unwrap_or_default();
            let a = Announce { id: s.pc_id, name: s.pc_name, port: tcp_port(), macs: store::macs(), version: 1 };
            let _ = sock.send_to(&serde_json::to_vec(&a).unwrap_or_default(), from);
        }
    });
}

pub fn listen(shared: Shared) -> std::io::Result<()> {
    let l = TcpListener::bind(("0.0.0.0", tcp_port()))?;
    for c in l.incoming().flatten() {
        let sh = shared.clone();
        std::thread::spawn(move || {
            let _ = c.set_nodelay(true);
            if let Err(e) = handle(c, sh) {
                eprintln!("oxide-remoted: connection ended: {e}");
            }
        });
    }
    Ok(())
}

fn refuse(c: &mut TcpStream, reason: &str) -> std::io::Result<()> {
    write_hello(c, &Hello::Refused { reason: reason.into() })
}

fn handle(mut c: TcpStream, sh: Shared) -> std::io::Result<()> {
    let local = c.peer_addr().map(|a| is_local(a.ip())).unwrap_or(false);
    if local {
        c.set_read_timeout(Some(Duration::from_secs(120)))?;
        return match read_hello(&mut c)? {
            Hello::PairHello { phone_id, phone_name, epk } => pair(c, sh, phone_id, phone_name, epk),
            Hello::Hello { phone_id, nonce } => session(c, sh, phone_id, nonce),
            _ => refuse(&mut c, "unexpected message"),
        };
    }
    // from the internet: only paired phones, a short handshake, a few at a time, never pairing
    if !sh.reach.lock().map(|r| r.on).unwrap_or(false) {
        return Ok(());
    }
    use std::sync::atomic::Ordering::SeqCst;
    if sh.remote_handshakes.fetch_add(1, SeqCst) >= 6 {
        sh.remote_handshakes.fetch_sub(1, SeqCst);
        return Ok(());
    }
    c.set_read_timeout(Some(Duration::from_secs(8)))?;
    let hello = read_hello(&mut c);
    sh.remote_handshakes.fetch_sub(1, SeqCst);
    match hello? {
        Hello::Hello { phone_id, nonce } => session(c, sh, phone_id, nonce),
        Hello::PairHello { .. } => refuse(&mut c, "Pairing works only on the same network as the PC."),
        _ => Ok(()),
    }
}

fn pair(mut c: TcpStream, sh: Shared, phone_id: String, phone_name: String, epk: String) -> std::io::Result<()> {
    let code = {
        let p = sh.pairing.lock().unwrap();
        p.code.as_ref().filter(|(_, until)| Instant::now() < *until).map(|(c, _)| c.clone())
    };
    let Some(code) = code else { return refuse(&mut c, "This PC is not waiting for a phone. Open Settings > Phone on the PC and press \"Pair a phone\".") };
    let Some(phone_epk) = unhex(&epk).and_then(|v| <[u8; 32]>::try_from(v).ok()) else { return refuse(&mut c, "bad key") };
    let key = PairKey::new();
    let s = sh.store.lock().map(|s| s.clone()).unwrap_or_default();
    write_hello(&mut c, &Hello::PairChallenge { pc_id: s.pc_id.clone(), pc_name: s.pc_name.clone(), epk: hex(&key.public), macs: store::macs() })?;
    let Hello::PairProof { mac } = read_hello(&mut c)? else { return refuse(&mut c, "unexpected message") };
    let shared = key.shared(&phone_epk);
    let (confirm, ltk) = pair_keys(&shared, &code, &phone_epk, &key.public);
    if !check_proof(&confirm, "phone", &mac) {
        let mut p = sh.pairing.lock().unwrap();
        p.failures += 1;
        if p.failures >= 3 {
            p.code = None;
        }
        return refuse(&mut c, "Wrong code.");
    }
    {
        let mut st = sh.store.lock().unwrap();
        st.phones.retain(|p| p.id != phone_id);
        let ip = c.peer_addr().map(|a| a.ip().to_string()).unwrap_or_default();
        st.phones.push(Phone { id: phone_id, name: phone_name.clone(), ltk: hex(&ltk), paired: store::now(), last_seen: store::now(), ip });
        st.save();
        sh.pairing.lock().unwrap().code = None;
    }
    if let Ok(mut l) = sh.log.lock() {
        l.push(format!("{phone_name} was paired."));
    }
    write_hello(&mut c, &Hello::PairOk { mac: proof(&confirm, "pc") })
}

/// Sends encrypted messages; shared by the request loop and the screen thread.
struct Sender {
    stream: TcpStream,
    cipher: Cipher,
}

impl Sender {
    fn send(&mut self, body: &[u8]) -> std::io::Result<()> {
        let sealed = self.cipher.seal(body);
        write_frame(&mut self.stream, &sealed)
    }
    fn json(&mut self, m: &ToPhone) -> std::io::Result<()> {
        self.send(&json_body(m))
    }
}

fn session(mut c: TcpStream, sh: Shared, phone_id: String, nonce: String) -> std::io::Result<()> {
    let s = sh.store.lock().map(|s| s.clone()).unwrap_or_default();
    let Some(phone) = s.phone(&phone_id).cloned() else { return refuse(&mut c, "This phone is not paired with this PC (any more). Pair it again.") };
    let (Some(ltk), Some(phone_nonce)) = (unhex(&phone.ltk).and_then(|v| <[u8; 32]>::try_from(v).ok()), unhex(&nonce)) else { return refuse(&mut c, "bad key") };
    let pc_nonce = random::<16>();
    write_hello(&mut c, &Hello::HelloOk { nonce: hex(&pc_nonce), pc_name: s.pc_name.clone(), pc_id: s.pc_id.clone() })?;
    let (mut from_phone, to_phone) = oxide_remote_proto::session(&ltk, &phone_nonce, &pc_nonce);
    if let Ok(mut st) = sh.store.lock() {
        if let Some(p) = st.phones.iter_mut().find(|p| p.id == phone_id) {
            p.last_seen = store::now();
            if let Ok(a) = c.peer_addr() {
                if is_local(a.ip()) {
                    p.ip = a.ip().to_string();
                }
            }
        }
        st.save();
    }
    c.set_read_timeout(Some(Duration::from_secs(60)))?;
    let sender = Arc::new(Mutex::new(Sender { stream: c.try_clone()?, cipher: to_phone }));
    // where to find this PC away from home, for next time
    let r = sh.reach.lock().map(|r| r.clone()).unwrap_or_default();
    let _ = sender.lock().unwrap().json(&ToPhone::Whereabouts(r.whereabouts()));
    let streaming = Arc::new(AtomicBool::new(false));
    let acked = Arc::new(AtomicU64::new(0));
    let mut watching = false;
    let result = (|| -> std::io::Result<()> {
        loop {
            let sealed = read_frame(&mut c)?;
            let body = from_phone.open(&sealed).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad frame (wrong key?)"))?;
            if body.first() != Some(&KIND_JSON) {
                continue;
            }
            let Ok(msg) = serde_json::from_slice::<ToPc>(&body[1..]) else { continue };
            let reply = |m: ToPhone| sender.lock().unwrap().json(&m);
            let done = |r: Result<(), String>| {
                let (ok, message) = match r {
                    Ok(()) => (true, String::new()),
                    Err(e) => (false, e),
                };
                sender.lock().unwrap().json(&ToPhone::Done { ok, message })
            };
            match msg {
                ToPc::Ping => reply(ToPhone::Pong)?,
                ToPc::Info => {
                    reply(ToPhone::Info(oxide_remote_proto::PcInfo {
                        name: s.pc_name.clone(),
                        hostname: store::hostname(),
                        os: plat::os_name(),
                        macs: store::macs(),
                        can_capture: true,
                        outputs: vec!["Screen".into()],
                        oxide: false,
                        palette: vec![],
                    }))?
                }
                ToPc::Stats => {
                    if !watching {
                        watching = true;
                        sh.sampler.watchers.fetch_add(1, Ordering::Relaxed);
                    }
                    let st = sh.sampler.latest.lock().map(|s| s.clone()).unwrap_or_default();
                    reply(ToPhone::Stats(st))?
                }
                ToPc::ScreenStart { max_w, quality, fps, output: _ } => {
                    if !streaming.swap(true, Ordering::SeqCst) {
                        stream_screen(sender.clone(), streaming.clone(), acked.clone(), max_w, quality, fps);
                    }
                }
                ToPc::ScreenStop => streaming.store(false, Ordering::SeqCst),
                ToPc::FrameAck { seq } => {
                    acked.fetch_max(seq, Ordering::SeqCst);
                }
                ToPc::PointerTo { x, y } => plat::pointer_to(x, y),
                ToPc::PointerBy { dx, dy } => plat::pointer_by(dx, dy),
                ToPc::Click { button } => {
                    plat::button(&button, true);
                    plat::button(&button, false);
                }
                ToPc::ButtonState { button, pressed } => plat::button(&button, pressed),
                ToPc::Scroll { dx, dy } => plat::scroll(dx, dy),
                ToPc::Key { name, mods } => done(plat::key(&name, &mods))?,
                ToPc::Text { text } => done(plat::text(&text))?,
                ToPc::Action { name, arg } => done(plat::action(&name, arg.as_deref()))?,
                ToPc::ClipboardGet => reply(ToPhone::Clipboard { text: arboard::Clipboard::new().and_then(|mut c| c.get_text()).unwrap_or_default() })?,
                // the assistant lives on the Linux desktop
                ToPc::Ask { .. } => done(Err("The assistant is only available on Oxide (Linux) PCs.".into()))?,
                ToPc::AskConfirm { .. } | ToPc::AskCancel | ToPc::AskReset => {}
            }
        }
    })();
    streaming.store(false, Ordering::SeqCst);
    if watching {
        sh.sampler.watchers.fetch_sub(1, Ordering::Relaxed);
    }
    result
}

/// Capture, shrink, encode and send frames while `on`, with as little delay as possible:
/// * capturing and encoding run on two threads, joined by a one-slot mailbox: the capture thread always replaces the
///   waiting picture, so the encoder takes the newest one and an old picture is never sent;
/// * at most two frames are in flight (the phone acknowledges every frame it showed), so a slow link never builds a
///   queue (it just lowers the frame rate);
/// * on Windows the screen is only captured when it changed (DXGI), so a still screen costs nothing.
#[allow(clippy::too_many_arguments)]
fn stream_screen(sender: Arc<Mutex<Sender>>, on: Arc<AtomicBool>, acked: Arc<AtomicU64>, max_w: u32, quality: u8, fps: u8) {
    use std::sync::Condvar;
    let slot: Arc<(Mutex<Option<plat::Image>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
    let sent = Arc::new(AtomicU64::new(acked.load(Ordering::SeqCst)));
    // encoder: JPEG and send
    {
        let (sender, on, acked, sent, slot) = (sender.clone(), on.clone(), acked.clone(), sent.clone(), slot.clone());
        std::thread::spawn(move || {
            let debug = std::env::var_os("OXIDE_REMOTE_DEBUG").is_some();
            let mut seq = acked.load(Ordering::SeqCst);
            while on.load(Ordering::SeqCst) {
                let img = {
                    let (m, cv) = &*slot;
                    let mut g = m.lock().unwrap();
                    loop {
                        if let Some(i) = g.take() {
                            break Some(i);
                        }
                        if !on.load(Ordering::SeqCst) {
                            break None;
                        }
                        g = cv.wait_timeout(g, Duration::from_millis(200)).unwrap().0;
                    }
                };
                let Some(img) = img else { break };
                let te = Instant::now();
                seq += 1;
                let f = Frame { seq, width: img.full.0, height: img.full.1, jpeg: plat::jpeg(&img, quality) };
                if debug {
                    eprintln!("frame {seq}: encode {} ms, {} KB", te.elapsed().as_millis(), f.jpeg.len() / 1024);
                }
                if sender.lock().unwrap().send(&f.encode()).is_err() {
                    on.store(false, Ordering::SeqCst);
                    break;
                }
                sent.store(seq, Ordering::SeqCst);
            }
        });
    }
    std::thread::spawn(move || {
        let mut cap = match Capturer::new() {
            Ok(c) => c,
            Err(e) => {
                let _ = sender.lock().unwrap().json(&ToPhone::Done { ok: false, message: e });
                on.store(false, Ordering::SeqCst);
                return;
            }
        };
        let debug = std::env::var_os("OXIDE_REMOTE_DEBUG").is_some();
        let period = Duration::from_millis(1000 / fps.clamp(1, 120) as u64);
        let mut last = Instant::now() - Duration::from_secs(10);
        while on.load(Ordering::SeqCst) {
            let t0 = Instant::now();
            // the phone has not shown the last two frames yet: wait instead of piling up
            if sent.load(Ordering::SeqCst).saturating_sub(acked.load(Ordering::SeqCst)) >= 2 && last.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            }
            // not faster than the wanted frame rate
            if last.elapsed() < period {
                std::thread::sleep(period - last.elapsed());
            }
            match cap.grab(max_w) {
                Ok(Some(img)) => {
                    if debug {
                        eprintln!("grab {} ms", t0.elapsed().as_millis());
                    }
                    let (m, cv) = &*slot;
                    *m.lock().unwrap() = Some(img);
                    cv.notify_one();
                    last = Instant::now();
                }
                Ok(None) => {}
                Err(e) => {
                    // the capture is lost (lock screen, another resolution): try again
                    let _ = sender.lock().unwrap().json(&ToPhone::Done { ok: false, message: e });
                    std::thread::sleep(Duration::from_millis(800));
                    if let Ok(c) = Capturer::new() {
                        cap = c;
                    }
                }
            }
        }
        slot.1.notify_all();
        let _ = sender.lock().map(|mut s| s.stream.flush());
    });
}
