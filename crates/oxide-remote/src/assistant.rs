//! The phone talks to the PC's assistant: one `oxide-ai` per phone session, started with the first question. Its JSON
//! lines go to the phone as they come (so the answer appears word by word) and the phone's replies (questions,
//! confirmations) go in. The phone is paired, so it may do what the user may do on the PC; the assistant asks for
//! a confirmation (shown on the phone) before anything risky, as on the PC.
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
};

use oxide_remote_proto::ToPhone;

use crate::server::Sender;

#[derive(Default)]
pub struct Assistant {
    child: Option<(Child, ChildStdin)>,
}

impl Assistant {
    fn start(&mut self, sender: &Arc<Mutex<Sender>>) -> Result<(), String> {
        let mut child = Command::new("oxide-ai")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("The assistant is not available on this PC ({e}). Is oxide-ai installed?"))?;
        let stdin = child.stdin.take().ok_or("no input")?;
        let out = child.stdout.take().ok_or("no output")?;
        let sender = sender.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if sender.lock().unwrap().json(&ToPhone::Assistant { json: line }).is_err() {
                    break;
                }
            }
        });
        self.child = Some((child, stdin));
        Ok(())
    }

    /// Hand the assistant one message (starting it when needed).
    pub fn send(&mut self, sender: &Arc<Mutex<Sender>>, msg: serde_json::Value) -> Result<(), String> {
        if self.child.as_mut().is_some_and(|(c, _)| c.try_wait().ok().flatten().is_some()) {
            self.child = None;
        }
        if self.child.is_none() {
            self.start(sender)?;
        }
        let (_, stdin) = self.child.as_mut().ok_or("assistant not running")?;
        writeln!(stdin, "{msg}").and_then(|_| stdin.flush()).map_err(|e| e.to_string())
    }

    pub fn stop(&mut self) {
        if let Some((mut c, _)) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
