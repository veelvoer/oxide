//! The phone side of the watch bridge on a computer, over TCP (for developing the watch app without Bluetooth):
//! `cargo run --example bridge_tcp -- <data dir> [port]`; the data dir holds pcs.json like the phone's.
fn main() {
    let mut a = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(a.next().expect("data dir"));
    let port: u16 = a.next().and_then(|p| p.parse().ok()).unwrap_or(47900);
    let l = std::net::TcpListener::bind(("127.0.0.1", port)).expect("port");
    println!("bridge on 127.0.0.1:{port}; pairing code: {}", oxide_remote::bridge::start_pairing());
    for s in l.incoming().flatten() {
        let (d, w) = (dir.clone(), s.try_clone().expect("clone"));
        std::thread::spawn(move || oxide_remote::bridge::serve(&d, "Test phone", s, w));
    }
}
