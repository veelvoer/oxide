//! Talking to PCs: discovery, pairing, the encrypted session (a reader and a writer thread), reachability.
use std::{
    io::Read,
    net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket},
    sync::mpsc::{channel, Receiver, Sender},
    time::{Duration, Instant},
};

use oxide_remote_proto::*;

use crate::store::Pc;

/// Look for PCs on the network for `wait`.
pub fn discover(wait: Duration) -> Vec<(Announce, String)> {
    let mut found: Vec<(Announce, String)> = vec![];
    let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else { return found };
    let _ = sock.set_broadcast(true);
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    let start = Instant::now();
    let mut buf = [0u8; 2048];
    let mut sent = 0;
    while start.elapsed() < wait {
        // send a few times: a single UDP packet gets lost on busy Wi-Fi
        if sent < 3 && start.elapsed() >= Duration::from_millis(sent * 400) {
            let _ = sock.send_to(DISCOVER, ("255.255.255.255", DISCOVERY_PORT));
            sent += 1;
        }
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            if let Ok(a) = serde_json::from_slice::<Announce>(&buf[..n]) {
                let host = from.ip().to_string();
                if !found.iter().any(|(f, _)| f.id == a.id) {
                    found.push((a, host));
                }
            }
        }
    }
    found
}

fn connect(host: &str, port: u16, timeout: Duration) -> std::io::Result<TcpStream> {
    let addr: SocketAddr = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "unknown address"))?;
    connect_addr(addr, timeout)
}

fn connect_addr(addr: SocketAddr, timeout: Duration) -> std::io::Result<TcpStream> {
    let s = TcpStream::connect_timeout(&addr, timeout)?;
    s.set_nodelay(true)?;
    Ok(s)
}

/// How the phone reaches a PC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// the same network
    Home,
    /// over the internet (or a VPN)
    Away,
}

/// Try addresses at the same time; the first that answers wins.
fn race(addrs: &[String], timeout: Duration) -> Option<TcpStream> {
    let (tx, rx) = channel();
    let mut n = 0;
    for a in addrs {
        let Some(addr) = a.to_socket_addrs().ok().and_then(|mut i| i.next()) else { continue };
        let tx = tx.clone();
        n += 1;
        std::thread::spawn(move || {
            let _ = tx.send(connect_addr(addr, timeout).ok());
        });
    }
    drop(tx);
    for _ in 0..n {
        match rx.recv() {
            Ok(Some(s)) => return Some(s),
            Ok(None) => continue,
            Err(_) => break,
        }
    }
    None
}

/// The PC's latest note on its rendezvous server: where it is now.
pub fn whereabouts(pc: &Pc) -> Option<Whereabouts> {
    let ltk: [u8; 32] = unhex(&pc.ltk)?.try_into().ok()?;
    let server = if pc.rendezvous.is_empty() { return None } else { pc.rendezvous.trim_end_matches('/') };
    let body = crate::platform::http_get(&format!("{server}/{}/json?poll=1&since=48h", note_topic(&ltk)))?;
    body.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["event"] == "message")
        .filter_map(|v| note_open(&ltk, v["message"].as_str()?))
        .filter_map(|b| serde_json::from_slice::<Whereabouts>(&b).ok())
        .max_by_key(|w| w.at)
}

/// Reach the PC: on the home network first, then at its addresses on the internet, then wherever its latest note
/// says it is. Returns the new addresses when they came from a note (to remember them).
pub fn connect_any(pc: &Pc) -> Result<(TcpStream, Route, Option<Whereabouts>), String> {
    if let Ok(s) = connect(&pc.host, pc.port, Duration::from_millis(1200)) {
        return Ok((s, Route::Home, None));
    }
    if let Some(s) = race(&pc.remote, Duration::from_secs(4)) {
        return Ok((s, Route::Away, None));
    }
    if let Some(w) = whereabouts(pc).filter(|w| w.addrs != pc.remote) {
        if let Some(s) = race(&w.addrs, Duration::from_secs(5)) {
            return Ok((s, Route::Away, Some(w)));
        }
    }
    Err(if pc.remote.is_empty() {
        format!("{} is not on this network. To reach it from anywhere, turn on \"Reach this PC from anywhere\" in its Settings > Phone and connect once at home.", pc.name)
    } else {
        format!("Cannot reach {}. Is it on?", pc.name)
    })
}

/// Whether the PC answers, and how (for the status on the list).
pub fn reachable(pc: &Pc) -> Option<Route> {
    if connect(&pc.host, pc.port, Duration::from_millis(700)).is_ok() {
        return Some(Route::Home);
    }
    race(&pc.remote, Duration::from_millis(2500)).map(|_| Route::Away)
}

// ------------------------------------------------------------- pairing

pub enum PairEvent {
    /// the PC answered: type the code it shows
    NeedCode { pc_name: String },
    Done(Result<Pc, String>),
}

/// Pair with the PC at host:port. Events come back on the returned receiver; send the code on the sender.
pub fn pair(host: String, port: u16, phone_id: String, phone_name: String) -> (Receiver<PairEvent>, Sender<String>) {
    let (ev_tx, ev_rx) = channel();
    let (code_tx, code_rx) = channel::<String>();
    std::thread::spawn(move || {
        let r = (|| -> Result<Pc, String> {
            let mut c = connect(&host, port, Duration::from_secs(4)).map_err(|e| format!("Cannot reach the PC: {e}"))?;
            let key = PairKey::new();
            write_hello(&mut c, &Hello::PairHello { phone_id, phone_name, epk: hex(&key.public) }).map_err(|e| e.to_string())?;
            let (pc_id, pc_name, pc_epk, macs) = match read_hello(&mut c).map_err(|e| e.to_string())? {
                Hello::PairChallenge { pc_id, pc_name, epk, macs } => (pc_id, pc_name, epk, macs),
                Hello::Refused { reason } => return Err(reason),
                _ => return Err("unexpected answer".into()),
            };
            let pc_epk: [u8; 32] = unhex(&pc_epk).and_then(|v| v.try_into().ok()).ok_or("bad key")?;
            let _ = ev_tx.send(PairEvent::NeedCode { pc_name: pc_name.clone() });
            let code = code_rx.recv_timeout(Duration::from_secs(115)).map_err(|_| "No code entered in time.".to_string())?;
            let shared = key.shared(&pc_epk);
            let (confirm, ltk) = pair_keys(&shared, code.trim(), &key.public, &pc_epk);
            write_hello(&mut c, &Hello::PairProof { mac: proof(&confirm, "phone") }).map_err(|e| e.to_string())?;
            match read_hello(&mut c).map_err(|e| e.to_string())? {
                Hello::PairOk { mac } if check_proof(&confirm, "pc", &mac) => Ok(Pc { id: pc_id, name: pc_name, host, port, macs, ltk: hex(&ltk), ..Default::default() }),
                Hello::PairOk { .. } => Err("The PC could not prove it knows the code.".into()),
                Hello::Refused { reason } => Err(reason),
                _ => Err("unexpected answer".into()),
            }
        })();
        let _ = ev_tx.send(PairEvent::Done(r));
    });
    (ev_rx, code_tx)
}

// ------------------------------------------------------------- session

pub enum Event {
    Connected { pc_name: String, route: Route },
    /// the PC's addresses changed (learned from its note): remember them
    Moved(Whereabouts),
    Msg(ToPhone),
    /// a decoded screen frame (RGBA) with the PC screen's full size
    Frame { seq: u64, rgba: Vec<u8>, w: usize, h: usize, full: (u32, u32), bytes: usize },
    Closed(String),
}

pub struct Session {
    pub to_pc: Sender<ToPc>,
    pub events: Receiver<Event>,
}

/// Open an encrypted session with `pc`. `wake` is called whenever an event arrives (to repaint the UI).
pub fn open(pc: &Pc, phone_id: &str, wake: impl Fn() + Send + Sync + 'static) -> Session {
    let (to_pc, out_rx) = channel::<ToPc>();
    let (ev_tx, events) = channel();
    let pc = pc.clone();
    let phone_id = phone_id.to_string();
    let wake = std::sync::Arc::new(wake);
    std::thread::spawn(move || {
        let r = (|| -> Result<(), String> {
            let (mut c, route, moved) = connect_any(&pc)?;
            if let Some(w) = moved {
                let _ = ev_tx.send(Event::Moved(w));
            }
            let ltk: [u8; 32] = unhex(&pc.ltk).and_then(|v| v.try_into().ok()).ok_or("broken pairing; pair again")?;
            let nonce = random::<16>();
            write_hello(&mut c, &Hello::Hello { phone_id, nonce: hex(&nonce) }).map_err(|e| e.to_string())?;
            let (pc_nonce, pc_name) = match read_hello(&mut c).map_err(|e| e.to_string())? {
                Hello::HelloOk { nonce, pc_name, .. } => (unhex(&nonce).ok_or("bad nonce")?, pc_name),
                Hello::Refused { reason } => return Err(reason),
                _ => return Err("unexpected answer".into()),
            };
            let (mut seal, mut open) = session(&ltk, &nonce, &pc_nonce);
            let _ = ev_tx.send(Event::Connected { pc_name, route });
            wake();
            // writer: everything the UI sends
            let mut w = c.try_clone().map_err(|e| e.to_string())?;
            std::thread::spawn(move || {
                while let Ok(m) = out_rx.recv() {
                    let sealed = seal.seal(&json_body(&m));
                    if write_frame(&mut w, &sealed).is_err() {
                        break;
                    }
                }
                let _ = w.shutdown(std::net::Shutdown::Both);
            });
            // reader: answers and frames
            loop {
                let sealed = read_frame(&mut c).map_err(|e| format!("Connection lost: {e}"))?;
                let body = open.open(&sealed).ok_or("The PC's key does not match. Pair again.")?;
                match body.first() {
                    Some(&KIND_JSON) => {
                        if let Ok(m) = serde_json::from_slice::<ToPhone>(&body[1..]) {
                            let _ = ev_tx.send(Event::Msg(m));
                        }
                    }
                    Some(&KIND_FRAME) => {
                        if let Some(f) = Frame::decode(&body) {
                            let bytes = f.jpeg.len();
                            if let Ok(img) = image::load_from_memory_with_format(&f.jpeg, image::ImageFormat::Jpeg) {
                                let img = img.to_rgba8();
                                let (w, h) = (img.width() as usize, img.height() as usize);
                                let _ = ev_tx.send(Event::Frame { seq: f.seq, rgba: img.into_raw(), w, h, full: (f.width, f.height), bytes });
                            }
                        }
                    }
                    _ => {}
                }
                wake();
            }
        })();
        let _ = ev_tx.send(Event::Closed(r.err().unwrap_or_else(|| "closed".into())));
        wake();
    });
    Session { to_pc, events }
}

/// Send a few messages in a short session (for example: ask one PC to wake another).
pub fn one_shot(pc: &Pc, phone_id: &str, msgs: Vec<ToPc>) -> Result<(), String> {
    let (mut c, _, _) = connect_any(pc)?;
    let ltk: [u8; 32] = unhex(&pc.ltk).and_then(|v| v.try_into().ok()).ok_or("broken pairing")?;
    let nonce = random::<16>();
    write_hello(&mut c, &Hello::Hello { phone_id: phone_id.to_string(), nonce: hex(&nonce) }).map_err(|e| e.to_string())?;
    let pc_nonce = match read_hello(&mut c).map_err(|e| e.to_string())? {
        Hello::HelloOk { nonce, .. } => unhex(&nonce).ok_or("bad nonce")?,
        Hello::Refused { reason } => return Err(reason),
        _ => return Err("unexpected answer".into()),
    };
    let (mut seal, _) = session(&ltk, &nonce, &pc_nonce);
    for m in msgs {
        write_frame(&mut c, &seal.seal(&json_body(&m))).map_err(|e| e.to_string())?;
    }
    // give the PC a moment to act before the connection closes
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

/// Read and drop everything (used to make sure a stream is drained) — kept small on purpose.
#[allow(dead_code)]
pub fn drain(mut r: impl Read) {
    let mut b = [0u8; 4096];
    while matches!(r.read(&mut b), Ok(n) if n > 0) {}
}
