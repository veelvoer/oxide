//! The watch bridge: a watch talks to this phone over Bluetooth, the phone keeps the encrypted sessions with the PCs and
//! passes everything on, so the watch needs no Wi-Fi at all (a watch on Wi-Fi drains its battery). The Bluetooth sockets
//! are in `BridgeService.java`; the conversation is here and works on any `Read`/`Write` (so it is tested without a phone).
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use oxide_remote_proto::{
    hex, random,
    watch::{read_msg, write_msg, WatchPc, WatchReq, WatchResp},
    ToPc, ToPhone,
};
use serde::{Deserialize, Serialize};

use crate::{
    net::{self, Event, Route},
    store::Store,
};

// ------------------------------------------------------------------ paired watches

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Watch {
    pub name: String,
    pub token: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Watches {
    list: Vec<Watch>,
}

fn watches_file(dir: &Path) -> PathBuf {
    dir.join("watches.json")
}

pub fn watches(dir: &Path) -> Vec<Watch> {
    std::fs::read_to_string(watches_file(dir)).ok().and_then(|t| serde_json::from_str::<Watches>(&t).ok()).unwrap_or_default().list
}

fn save_watches(dir: &Path, list: Vec<Watch>) {
    let tmp = dir.join("watches.json.tmp");
    if std::fs::write(&tmp, serde_json::to_string_pretty(&Watches { list }).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, watches_file(dir));
    }
}

pub fn forget_watch(dir: &Path, token: &str) {
    let list = watches(dir).into_iter().filter(|w| w.token != token).collect();
    save_watches(dir, list);
}

/// The 6-digit code shown on the phone while "Pair a watch" is open, and until when.
static PAIRING: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub fn start_pairing() -> String {
    let code = format!("{:06}", u32::from_be_bytes(random::<4>()) % 1_000_000);
    *PAIRING.lock().unwrap() = Some((code.clone(), Instant::now() + Duration::from_secs(120)));
    code
}

pub fn stop_pairing() {
    *PAIRING.lock().unwrap() = None;
}

/// The code and the seconds it is still valid.
pub fn pairing_code() -> Option<(String, u64)> {
    let mut p = PAIRING.lock().unwrap();
    match p.as_ref() {
        Some((c, until)) if Instant::now() < *until => Some((c.clone(), until.saturating_duration_since(Instant::now()).as_secs())),
        Some(_) => {
            *p = None;
            None
        }
        None => None,
    }
}

/// Wrong guesses since the code was made: three and the code is void (it is only 6 digits).
static TRIES: AtomicU64 = AtomicU64::new(0);

fn try_pair(dir: &Path, name: &str, code: &str) -> Result<String, String> {
    let Some((real, _)) = pairing_code() else { return Err("The phone is not waiting for a watch. Press \"Pair a watch\" on the phone.".into()) };
    if code.trim() != real {
        if TRIES.fetch_add(1, Ordering::SeqCst) + 1 >= 3 {
            stop_pairing();
            TRIES.store(0, Ordering::SeqCst);
            return Err("Wrong code three times. Start again on the phone.".into());
        }
        return Err("Wrong code.".into());
    }
    TRIES.store(0, Ordering::SeqCst);
    stop_pairing();
    let token = hex(&random::<16>());
    let mut list = watches(dir);
    list.push(Watch { name: name.to_string(), token: token.clone() });
    save_watches(dir, list);
    Ok(token)
}

// ------------------------------------------------------------------ the conversation

type Out = Arc<Mutex<Box<dyn Write + Send>>>;

fn send(out: &Out, m: &WatchResp) -> bool {
    match out.lock() {
        Ok(mut w) => write_msg(&mut *w, m).is_ok(),
        Err(_) => false,
    }
}

fn state_of(r: Option<Option<Route>>) -> u8 {
    match r {
        None => 0,
        Some(Some(Route::Home)) => 1,
        Some(Some(Route::Away)) => 2,
        Some(None) => 3,
    }
}

fn pcs_list(store: &Store, states: &[Option<Option<Route>>]) -> Vec<WatchPc> {
    store.pcs.iter().enumerate().map(|(i, p)| WatchPc { id: p.id.clone(), name: p.name.clone(), state: state_of(states.get(i).copied().flatten()), palette: p.palette.clone() }).collect()
}

/// Serve one watch until the link drops. `phone_name` is what the watch shows.
pub fn serve(dir: &Path, phone_name: &str, mut input: impl Read, output: impl Write + Send + 'static) {
    let out: Out = Arc::new(Mutex::new(Box::new(output)));
    let mut authed = false;
    let mut watch_name = String::new();
    let session_gen = Arc::new(AtomicU64::new(0));
    let mut to_pc: Option<std::sync::mpsc::Sender<ToPc>> = None;
    let mut glide: Option<std::sync::mpsc::Sender<Pipe>> = None;
    loop {
        let Ok(req) = read_msg::<WatchReq>(&mut input) else { break };
        match req {
            WatchReq::Hello { name, token } => {
                if !token.is_empty() && watches(dir).iter().any(|w| w.token == token) {
                    authed = true;
                    watch_name = name;
                    send(&out, &WatchResp::Welcome { phone: phone_name.to_string() });
                } else {
                    send(&out, &WatchResp::NeedPairing);
                }
            }
            WatchReq::Pair { code } => match try_pair(dir, &watch_name_or(&watch_name), &code) {
                Ok(token) => {
                    authed = true;
                    send(&out, &WatchResp::Paired { token });
                    send(&out, &WatchResp::Welcome { phone: phone_name.to_string() });
                }
                Err(reason) => {
                    send(&out, &WatchResp::PairFailed { reason });
                }
            },
            _ if !authed => {
                send(&out, &WatchResp::NeedPairing);
            }
            WatchReq::Ping => {
                send(&out, &WatchResp::Pong);
            }
            WatchReq::List => list(dir, &out),
            WatchReq::Wake { id } => {
                let store = Store::load(dir.to_path_buf());
                let name = store.pcs.iter().find(|p| p.id == id).map(|p| p.name.clone());
                if let Some(pc) = store.pcs.iter().find(|p| p.id == id).cloned() {
                    wake(&store, &pc);
                    send(&out, &WatchResp::Toast { text: format!("Waking {}", name.unwrap_or_default()) });
                }
            }
            WatchReq::Open { id } => {
                let store = Store::load(dir.to_path_buf());
                let gen = session_gen.fetch_add(1, Ordering::SeqCst) + 1;
                to_pc = None;
                glide = None;
                let Some(pc) = store.pcs.iter().find(|p| p.id == id).cloned() else {
                    send(&out, &WatchResp::OpenFailed { error: "The phone does not know this PC any more.".into() });
                    continue;
                };
                let session = net::open(&pc, &store.phone_id, || {});
                glide = Some(pointer_pipe(session.to_pc.clone()));
                to_pc = Some(session.to_pc);
                let (out2, gen_now) = (out.clone(), session_gen.clone());
                std::thread::spawn(move || {
                    for ev in session.events {
                        if gen_now.load(Ordering::SeqCst) != gen {
                            break;
                        }
                        let resp = match ev {
                            Event::Connected { pc_name, .. } => Some(WatchResp::Opened { name: pc_name }),
                            Event::Msg(ToPhone::Whereabouts(_)) => None,
                            Event::Msg(m) => Some(WatchResp::Msg { msg: m }),
                            Event::Closed(reason) => Some(WatchResp::Closed { reason }),
                            Event::Moved(_) | Event::Frame { .. } => None,
                        };
                        if let Some(r) = resp {
                            if !send(&out2, &r) {
                                break;
                            }
                        }
                    }
                });
            }
            WatchReq::Close => {
                session_gen.fetch_add(1, Ordering::SeqCst);
                to_pc = None;
                glide = None;
            }
            WatchReq::Send { msg: ToPc::PointerBy { dx, dy } } => {
                if let Some(g) = &glide {
                    let _ = g.send(Pipe::Rel(dx, dy));
                }
            }
            WatchReq::Send { msg: ToPc::PointerTo { x, y } } => {
                if let Some(g) = &glide {
                    let _ = g.send(Pipe::Abs(x, y));
                }
            }
            WatchReq::Send { msg: msg @ (ToPc::Click { .. } | ToPc::ButtonState { .. } | ToPc::Scroll { .. }) } => {
                // through the pipe: the pointer arrives first
                match &glide {
                    Some(g) => {
                        let _ = g.send(Pipe::Pass(msg));
                    }
                    None => {
                        if let Some(tx) = &to_pc {
                            let _ = tx.send(msg);
                        }
                    }
                }
            }
            WatchReq::Send { msg } => {
                // the watch only drives the pointer, the keyboard and the actions: no screen frames
                if matches!(msg, ToPc::ScreenStart { .. }) {
                    continue;
                }
                if let Some(tx) = &to_pc {
                    let _ = tx.send(msg);
                }
            }
        }
    }
    session_gen.fetch_add(1, Ordering::SeqCst);
}

fn watch_name_or(n: &str) -> String {
    if n.is_empty() {
        "Watch".into()
    } else {
        n.to_string()
    }
}

/// What goes to the PC through the pointer pipe.
enum Pipe {
    /// a relative move (the touchpad, the gyro in its mouse mode)
    Rel(f64, f64),
    /// the pointer goes here (the gyro pointing at the screen), fractions of the screen
    Abs(f64, f64),
    /// anything else (a click...): what is still owed to the pointer is paid first, so a click lands where it was meant
    Pass(ToPc),
}

/// Pointer moves from the watch arrive in bursts (Bluetooth carries a few packets a second when it is idle). This hands
/// them to the PC spread over time instead, so the pointer glides: each tick pays a share of what is owed (relative) or
/// chases the target (absolute). A click flushes both first.
fn pointer_pipe(to_pc: std::sync::mpsc::Sender<ToPc>) -> std::sync::mpsc::Sender<Pipe> {
    let (tx, rx) = std::sync::mpsc::channel::<Pipe>();
    std::thread::spawn(move || {
        const TICK: Duration = Duration::from_millis(6);
        // relative: about 45 ms to pay what is owed. absolute: the pointer closes 30 % of the gap per tick
        let share = 1.0 - (-(TICK.as_secs_f64()) / 0.045).exp();
        let (mut ox, mut oy) = (0.0f64, 0.0f64);
        let mut target: Option<(f64, f64)> = None;
        let mut cur: Option<(f64, f64)> = None;
        let busy = |ox: f64, oy: f64, t: &Option<(f64, f64)>, c: &Option<(f64, f64)>| ox.abs() >= 0.01 || oy.abs() >= 0.01 || (t.is_some() && t != c);
        loop {
            let first = if !busy(ox, oy, &target, &cur) {
                match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => return,
                }
            } else {
                match rx.recv_timeout(TICK) {
                    Ok(m) => Some(m),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                    Err(_) => return,
                }
            };
            let mut msgs: Vec<Pipe> = first.into_iter().collect();
            while let Ok(m) = rx.try_recv() {
                msgs.push(m);
            }
            for m in msgs {
                match m {
                    Pipe::Rel(dx, dy) => {
                        ox += dx;
                        oy += dy;
                    }
                    Pipe::Abs(x, y) => target = Some((x, y)),
                    Pipe::Pass(msg) => {
                        // flush: the pointer is where it should be before the message goes
                        if ox != 0.0 || oy != 0.0 {
                            let _ = to_pc.send(ToPc::PointerBy { dx: ox, dy: oy });
                            ox = 0.0;
                            oy = 0.0;
                        }
                        if let Some(t) = target {
                            if cur != Some(t) {
                                let _ = to_pc.send(ToPc::PointerTo { x: t.0, y: t.1 });
                                cur = Some(t);
                            }
                        }
                        let _ = to_pc.send(msg);
                    }
                }
            }
            let (mx, my) = if ox.abs() < 0.6 && oy.abs() < 0.6 { (ox, oy) } else { (ox * share, oy * share) };
            ox -= mx;
            oy -= my;
            if (mx != 0.0 || my != 0.0) && to_pc.send(ToPc::PointerBy { dx: mx, dy: my }).is_err() {
                return;
            }
            if let Some(t) = target {
                let next = match cur {
                    None => t,
                    Some(c) => {
                        let (dx, dy) = (t.0 - c.0, t.1 - c.1);
                        if dx.abs() < 0.0004 && dy.abs() < 0.0004 {
                            t
                        } else {
                            (c.0 + dx * 0.3, c.1 + dy * 0.3)
                        }
                    }
                };
                if cur != Some(next) {
                    cur = Some(next);
                    if to_pc.send(ToPc::PointerTo { x: next.0, y: next.1 }).is_err() {
                        return;
                    }
                }
            }
        }
    });
    tx
}

/// The PCs at once (state unknown), then again as each one answers.
fn list(dir: &Path, out: &Out) {
    let store = Store::load(dir.to_path_buf());
    let n = store.pcs.len();
    let mut states: Vec<Option<Option<Route>>> = vec![None; n];
    send(out, &WatchResp::Pcs { pcs: pcs_list(&store, &states) });
    if n == 0 {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    for (i, pc) in store.pcs.iter().cloned().enumerate() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((i, net::reachable(&pc).map(|(r, _)| r)));
        });
    }
    drop(tx);
    let out = out.clone();
    std::thread::spawn(move || {
        for (i, r) in rx {
            states[i] = Some(r);
            if !send(&out, &WatchResp::Pcs { pcs: pcs_list(&store, &states) }) {
                break;
            }
        }
    });
}

fn wake(store: &Store, pc: &crate::store::Pc) {
    for m in &pc.macs {
        let _ = oxide_remote_proto::wake(m, pc.subnet_broadcast());
    }
    // through the other PCs, for when the phone is not on this PC's network
    for h in store.pcs.iter().filter(|p| p.id != pc.id).cloned() {
        let msgs = pc.macs.iter().map(|m| ToPc::Action { name: "wake".into(), arg: Some(format!("{m} {}", pc.host)) }).collect();
        let id = store.phone_id.clone();
        std::thread::spawn(move || {
            let _ = net::one_shot(&h, &id, msgs);
        });
    }
}

// ------------------------------------------------------------------ Android glue

#[cfg(target_os = "android")]
mod glue {
    use std::{path::PathBuf, sync::Arc};

    use jni::{
        objects::{JClass, JObject, JString},
        JNIEnv,
    };

    use crate::jstream::JStream;

    /// `BridgeService.serve(InputStream, OutputStream, String dir, String phoneName)`: one watch, until the link drops.
    #[unsafe(no_mangle)]
    pub extern "system" fn Java_dev_oxide_remote_BridgeService_serve(mut env: JNIEnv, _c: JClass, input: JObject, output: JObject, dir: JString, name: JString) {
        let (Ok(vm), Ok(i), Ok(o)) = (env.get_java_vm(), env.new_global_ref(&input), env.new_global_ref(&output)) else { return };
        let vm = Arc::new(vm);
        let dir: String = env.get_string(&dir).map(Into::into).unwrap_or_default();
        let name: String = env.get_string(&name).map(Into::into).unwrap_or_default();
        // a panic must not unwind into Java (that would end the whole app): the watch's link just ends
        let dir = PathBuf::from(dir);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::serve(&dir, &name, JStream { vm: vm.clone(), obj: i }, JStream { vm, obj: o })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Receiver, Sender};

    /// A pipe: what one side writes, the other reads.
    struct PipeW(Sender<Vec<u8>>);
    struct PipeR {
        rx: Receiver<Vec<u8>>,
        buf: Vec<u8>,
    }
    impl Write for PipeW {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.send(b.to_vec()).map_err(|_| std::io::ErrorKind::BrokenPipe)?;
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Read for PipeR {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.buf.is_empty() {
                match self.rx.recv() {
                    Ok(b) => self.buf = b,
                    Err(_) => return Ok(0),
                }
            }
            let n = out.len().min(self.buf.len());
            out[..n].copy_from_slice(&self.buf[..n]);
            self.buf.drain(..n);
            Ok(n)
        }
    }
    fn pipe() -> (PipeW, PipeR) {
        let (tx, rx) = channel();
        (PipeW(tx), PipeR { rx, buf: vec![] })
    }

    #[test]
    fn a_click_waits_for_the_pointer() {
        let (tx, rx) = channel::<ToPc>();
        let pipe = pointer_pipe(tx);
        pipe.send(Pipe::Rel(40.0, 0.0)).unwrap();
        pipe.send(Pipe::Abs(0.25, 0.75)).unwrap();
        pipe.send(Pipe::Pass(ToPc::Click { button: "left".into() })).unwrap();
        let (mut dx, mut last) = (0.0, None);
        let mut abs = None;
        while let Ok(m) = rx.recv_timeout(Duration::from_millis(200)) {
            match m {
                ToPc::PointerBy { dx: d, .. } => dx += d,
                ToPc::PointerTo { x, y } => abs = Some((x, y)),
                ToPc::Click { .. } => {
                    last = Some((dx, abs));
                    break;
                }
                _ => {}
            }
        }
        let (dx, abs) = last.expect("the click arrived");
        assert!((dx - 40.0).abs() < 1e-6, "all that was owed was paid before the click, got {dx}");
        assert_eq!(abs, Some((0.25, 0.75)));
    }

    #[test]
    fn pairing_then_list() {
        let dir = std::env::temp_dir().join(format!("oxide-bridge-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (mut w_req, r_req) = pipe();
        let (w_resp, mut r_resp) = pipe();
        let d2 = dir.clone();
        let t = std::thread::spawn(move || serve(&d2, "TestPhone", r_req, w_resp));
        // unknown watch
        write_msg(&mut w_req, &WatchReq::Hello { name: "W".into(), token: String::new() }).unwrap();
        assert_eq!(read_msg::<WatchResp>(&mut r_resp).unwrap(), WatchResp::NeedPairing);
        // nothing is allowed before pairing
        write_msg(&mut w_req, &WatchReq::List).unwrap();
        assert_eq!(read_msg::<WatchResp>(&mut r_resp).unwrap(), WatchResp::NeedPairing);
        // a wrong code, then the right one
        let code = start_pairing();
        let wrong = if code == "000000" { "111111" } else { "000000" };
        write_msg(&mut w_req, &WatchReq::Pair { code: wrong.into() }).unwrap();
        assert!(matches!(read_msg::<WatchResp>(&mut r_resp).unwrap(), WatchResp::PairFailed { .. }));
        write_msg(&mut w_req, &WatchReq::Pair { code }).unwrap();
        let WatchResp::Paired { token } = read_msg::<WatchResp>(&mut r_resp).unwrap() else { panic!("not paired") };
        assert!(matches!(read_msg::<WatchResp>(&mut r_resp).unwrap(), WatchResp::Welcome { .. }));
        write_msg(&mut w_req, &WatchReq::List).unwrap();
        assert_eq!(read_msg::<WatchResp>(&mut r_resp).unwrap(), WatchResp::Pcs { pcs: vec![] });
        // the token works on a new link
        drop(w_req);
        t.join().unwrap();
        let (mut w_req, r_req) = pipe();
        let (w_resp, mut r_resp) = pipe();
        let d2 = dir.clone();
        let t = std::thread::spawn(move || serve(&d2, "TestPhone", r_req, w_resp));
        write_msg(&mut w_req, &WatchReq::Hello { name: "W".into(), token }).unwrap();
        assert!(matches!(read_msg::<WatchResp>(&mut r_resp).unwrap(), WatchResp::Welcome { .. }));
        drop(w_req);
        t.join().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
