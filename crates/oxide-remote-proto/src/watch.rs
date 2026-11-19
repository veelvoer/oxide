//! Watch <-> phone over Bluetooth. The watch has no network of its own in this mode: it asks the phone, which keeps the
//! encrypted sessions with the PCs and passes the messages on. Frames are a 4-byte big-endian length and a JSON body.
//! The Bluetooth link itself is encrypted by Android (the watch and phone are bonded); on top of that the watch must
//! have been paired once with a 6-digit code shown on the phone, and then proves itself with a token.
use std::io::{Read, Write};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{ToPc, ToPhone};

/// The RFCOMM service the phone listens on.
pub const SERVICE_UUID: &str = "5e0a6f1c-6b0d-4c0f-9d7c-0f3a4b2e9a11";
const MAX_MSG: usize = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum WatchReq {
    /// First message: the watch's name and its token (empty before it was paired).
    Hello { name: String, token: String },
    /// Pairing: the code the phone shows.
    Pair { code: String },
    /// The PCs the phone knows, with their state.
    List,
    Wake { id: String },
    /// Open a PC: from now on `Send` goes to it and its answers come back as `Msg`.
    Open { id: String },
    Close,
    Send { msg: ToPc },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WatchPc {
    pub id: String,
    pub name: String,
    /// 0 = looking, 1 = on the home network, 2 = over the internet, 3 = offline
    pub state: u8,
    /// the PC's Oxide colors (for the watch's "PC" theme)
    pub palette: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum WatchResp {
    /// The watch is known: the phone's name.
    Welcome { phone: String },
    /// Unknown watch: press "Pair a watch" on the phone and send `Pair` with its code.
    NeedPairing,
    Paired { token: String },
    PairFailed { reason: String },
    Pcs { pcs: Vec<WatchPc> },
    Opened { name: String },
    OpenFailed { error: String },
    Msg { msg: ToPhone },
    Closed { reason: String },
    Toast { text: String },
    Pong,
}

pub fn write_msg<T: Serialize>(w: &mut impl Write, m: &T) -> std::io::Result<()> {
    let body = serde_json::to_vec(m).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // one write: a frame must not be split across two Bluetooth packets by a second writer
    let mut buf = Vec::with_capacity(4 + body.len());
    buf.extend_from_slice(&(body.len() as u32).to_be_bytes());
    buf.extend_from_slice(&body);
    w.write_all(&buf)?;
    w.flush()
}

pub fn read_msg<T: DeserializeOwned>(r: &mut impl Read) -> std::io::Result<T> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let n = u32::from_be_bytes(len) as usize;
    if n > MAX_MSG {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "message too big"));
    }
    let mut body = vec![0u8; n];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        write_msg(&mut buf, &WatchReq::Send { msg: ToPc::PointerBy { dx: 1.5, dy: -2.0 } }).unwrap();
        write_msg(&mut buf, &WatchReq::List).unwrap();
        let mut r = std::io::Cursor::new(buf);
        assert_eq!(read_msg::<WatchReq>(&mut r).unwrap(), WatchReq::Send { msg: ToPc::PointerBy { dx: 1.5, dy: -2.0 } });
        assert_eq!(read_msg::<WatchReq>(&mut r).unwrap(), WatchReq::List);
    }
}
