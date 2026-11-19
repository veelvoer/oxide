//! The link to the phone over Bluetooth (the phone's Oxide Remote app does the networking). One background thread keeps
//! the link up: it looks for the phone, says hello, reads what comes back and writes what the app asks. Nothing here
//! touches Wi-Fi.
use std::{
    io::{Read, Write},
    sync::{
        mpsc::{channel, Receiver, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use oxide_remote_proto::watch::{read_msg, write_msg, WatchReq, WatchResp};

pub enum LinkEvent {
    /// what the link is doing while there is no phone ("Looking for your phone…", "Bluetooth is off")
    Status(String),
    /// the phone answered (its address, to look there first next time)
    Up(String),
    Down,
    Resp(WatchResp),
}

pub struct Link {
    pub tx: Sender<WatchReq>,
    pub rx: Receiver<LinkEvent>,
}

type Reader = Box<dyn Read + Send>;
type Writer = Box<dyn Write + Send>;

#[cfg(target_os = "android")]
mod transport {
    use std::sync::Arc;

    use jni::objects::{JObject, JString, JValue};

    use super::{Reader, Writer};
    use crate::{jstream::JStream, motion};

    pub fn connect(last: &str) -> Result<(Reader, Writer, String), String> {
        match motion::call(|env, a| env.call_method(a, "btState", "()I", &[])?.i()).unwrap_or(4) {
            1 => return Err("Allow Bluetooth for this app".into()),
            2 => return Err("Bluetooth is off".into()),
            3 => return Err("Pair the watch with your phone first".into()),
            4 => return Err("No Bluetooth".into()),
            _ => {}
        }
        let addr = motion::call(|env, a| {
            let js = env.new_string(last)?;
            let o = env.call_method(a, "btConnect", "(Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&js)])?.l()?;
            let s: String = env.get_string(&JString::from(o))?.into();
            Ok(s)
        })
        .unwrap_or_default();
        if addr.is_empty() {
            let why = motion::call(|env, a| {
                let o = env.call_method(a, "btError", "()Ljava/lang/String;", &[])?.l()?;
                let s: String = env.get_string(&JString::from(o))?.into();
                Ok(s)
            })
            .unwrap_or_default();
            return Err(format!("Looking for your phone. Is Oxide Remote on it, with \"Let my watch connect\"? {why}"));
        }
        let streams = motion::call(|env, a| {
            let vm = Arc::new(env.get_java_vm()?);
            let i: JObject = env.call_method(a, "btIn", "()Ljava/io/InputStream;", &[])?.l()?;
            let o: JObject = env.call_method(a, "btOut", "()Ljava/io/OutputStream;", &[])?.l()?;
            let (gi, go) = (env.new_global_ref(i)?, env.new_global_ref(o)?);
            Ok((JStream { vm: vm.clone(), obj: gi }, JStream { vm, obj: go }))
        });
        match streams {
            Some((i, o)) => Ok((Box::new(i), Box::new(o), addr)),
            None => Err("Could not open the Bluetooth link".into()),
        }
    }

    pub fn close() {
        motion::call(|env, a| env.call_method(a, "btClose", "()V", &[]).map(|_| ()));
    }
}

/// On a computer (for development): the phone side over TCP, `OXIDE_WATCH_BRIDGE=127.0.0.1:port`.
#[cfg(not(target_os = "android"))]
mod transport {
    use super::{Reader, Writer};

    pub fn connect(_last: &str) -> Result<(Reader, Writer, String), String> {
        let addr = std::env::var("OXIDE_WATCH_BRIDGE").map_err(|_| "Looking for your phone…".to_string())?;
        let s = std::net::TcpStream::connect(&addr).map_err(|e| format!("Phone not found: {e}"))?;
        let _ = s.set_nodelay(true);
        let r = s.try_clone().map_err(|e| e.to_string())?;
        Ok((Box::new(r), Box::new(s), addr))
    }

    pub fn close() {}
}

/// Start the link. `name` is the watch's name, `token` what it got when it was paired (empty before), `last` where the
/// phone was last time. `wake` repaints the screen when something arrived.
pub fn start(name: String, token: Arc<Mutex<String>>, last: String, wake: impl Fn() + Send + Sync + 'static) -> Link {
    let (tx, req_rx) = channel::<WatchReq>();
    let (ev_tx, rx) = channel::<LinkEvent>();
    let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(wake);
    std::thread::spawn(move || {
        let mut last = last;
        loop {
            let (reader, mut writer, addr) = match transport::connect(&last) {
                Ok(t) => t,
                Err(why) => {
                    let _ = ev_tx.send(LinkEvent::Status(why));
                    wake();
                    // nothing is waiting for the phone: look again in a few seconds (a watch must not spin)
                    match req_rx.recv_timeout(Duration::from_secs(5)) {
                        Err(RecvTimeoutError::Disconnected) => return,
                        _ => continue,
                    }
                }
            };
            last = addr.clone();
            let _ = ev_tx.send(LinkEvent::Up(addr));
            wake();
            let hello = WatchReq::Hello { name: name.clone(), token: token.lock().map(|t| t.clone()).unwrap_or_default() };
            if write_msg(&mut writer, &hello).is_err() {
                transport::close();
                continue;
            }
            // reader thread: everything the phone says goes to the app
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let (ev2, done2, wake2) = (ev_tx.clone(), done.clone(), wake.clone());
            let rd = std::thread::spawn(move || {
                let mut reader = reader;
                while let Ok(r) = read_msg::<WatchResp>(&mut reader) {
                    if ev2.send(LinkEvent::Resp(r)).is_err() {
                        break;
                    }
                    wake2();
                }
                done2.store(true, std::sync::atomic::Ordering::SeqCst);
            });
            // writer: what the app asks (the pointer moves at 60 per second when the touchpad is used)
            loop {
                match req_rx.recv_timeout(Duration::from_millis(250)) {
                    Ok(req) => {
                        if write_msg(&mut writer, &req).is_err() {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        if done.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        transport::close();
                        return;
                    }
                }
            }
            transport::close();
            let _ = rd.join();
            let _ = ev_tx.send(LinkEvent::Down);
            wake();
        }
    });
    Link { tx, rx }
}

