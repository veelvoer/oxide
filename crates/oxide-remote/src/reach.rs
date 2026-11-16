//! Away from home: make this PC reachable from the internet and tell paired phones where it is.
//!
//! * The router is asked to forward the TCP port to this PC (UPnP IGD, renewed every half hour, removed again when
//!   the option is turned off). Its answer also gives the public IPv4 address.
//! * Global IPv6 addresses and VPN addresses (Tailscale, ZeroTier: 100.64.0.0/10) are added as they are.
//! * Phones learn the addresses inside the encrypted session, and from an encrypted note left on a ntfy server
//!   whenever they change (topic and key come from each phone's pairing key, so only that phone can read it).
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    time::Duration,
};

use oxide_remote_proto::{note_seal, note_topic, unhex, Whereabouts, TCP_PORT};

use crate::store::{self, Store};

/// What the settings page shows and what phones are told.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Reach {
    pub on: bool,
    /// the router forwards this external port
    pub upnp_port: Option<u16>,
    pub public_ip: Option<String>,
    pub addrs: Vec<String>,
    pub problem: Option<String>,
    pub rendezvous: String,
    /// addresses on the network the PC is on now
    pub lan: Vec<String>,
}

impl Reach {
    pub fn whereabouts(&self) -> Whereabouts {
        Whereabouts { addrs: self.addrs.clone(), lan: self.lan.clone(), rendezvous: self.rendezvous.clone(), at: store::now() }
    }
}

#[derive(Clone)]
struct Gateway {
    host: String,
    port: u16,
    control: String,
    service: String,
    local_ip: Ipv4Addr,
}

// ------------------------------------------------------------- plain HTTP, enough for a router

fn http(host: &str, port: u16, request: &str, timeout: Duration) -> Option<(u16, String)> {
    let addr: SocketAddr = format!("{host}:{port}").parse().ok()?;
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?;
    s.write_all(request.as_bytes()).ok()?;
    let mut buf = vec![];
    let _ = s.take(512 * 1024).read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf).to_string();
    let code = text.split_whitespace().nth(1)?.parse().ok()?;
    let body = text.split_once("\r\n\r\n").map(|(_, b)| b.to_string()).unwrap_or_default();
    Some((code, body))
}

fn split_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = rest.split_once('/').map(|(h, p)| (h, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let (host, port) = hostport.rsplit_once(':').map(|(h, p)| (h.to_string(), p.parse().unwrap_or(80))).unwrap_or((hostport.to_string(), 80));
    Some((host, port, path))
}

fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let i = xml.find(&open)? + open.len();
    let j = xml[i..].find(&format!("</{name}>"))? + i;
    Some(xml[i..j].trim())
}

/// Find the router's port-forwarding service.
fn discover_gateway() -> Option<Gateway> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(Duration::from_millis(400))).ok()?;
    for st in ["urn:schemas-upnp-org:device:InternetGatewayDevice:1", "urn:schemas-upnp-org:device:InternetGatewayDevice:2"] {
        let msg = format!("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nST: {st}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\n\r\n");
        let _ = sock.send_to(msg.as_bytes(), "239.255.255.250:1900");
    }
    let mut buf = [0u8; 2048];
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(2500) {
        let Ok((n, _)) = sock.recv_from(&mut buf) else { continue };
        let text = String::from_utf8_lossy(&buf[..n]);
        let Some(loc) = text.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("location").then(|| v.trim().to_string())
        }) else {
            continue;
        };
        let Some((host, port, path)) = split_url(&loc) else { continue };
        let req = format!("GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
        let Some((200, xml)) = http(&host, port, &req, Duration::from_secs(3)) else { continue };
        // the first WAN connection service and its control URL
        for service in ["urn:schemas-upnp-org:service:WANIPConnection:2", "urn:schemas-upnp-org:service:WANIPConnection:1", "urn:schemas-upnp-org:service:WANPPPConnection:1"] {
            let Some(at) = xml.find(&format!("<serviceType>{service}</serviceType>")) else { continue };
            let block_end = xml[at..].find("</service>").map(|e| at + e).unwrap_or(xml.len());
            let Some(control) = tag(&xml[at..block_end], "controlURL") else { continue };
            let control = if control.starts_with("http://") {
                split_url(control).map(|(_, _, p)| p).unwrap_or_default()
            } else if control.starts_with('/') {
                control.to_string()
            } else {
                format!("/{control}")
            };
            // the address the router sees for this PC
            let local_ip = TcpStream::connect_timeout(&format!("{host}:{port}").parse().ok()?, Duration::from_secs(2))
                .ok()
                .and_then(|s| s.local_addr().ok())
                .and_then(|a| match a.ip() {
                    IpAddr::V4(v) => Some(v),
                    _ => None,
                })?;
            return Some(Gateway { host, port, control, service: service.into(), local_ip });
        }
    }
    None
}

fn soap(g: &Gateway, action: &str, args: &str) -> Result<String, String> {
    let body = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{}\">{args}</u:{action}></s:Body></s:Envelope>",
        g.service
    );
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: text/xml; charset=\"utf-8\"\r\nSOAPAction: \"{}#{action}\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        g.control,
        g.host,
        g.port,
        g.service,
        body.len()
    );
    match http(&g.host, g.port, &req, Duration::from_secs(4)) {
        Some((200, b)) => Ok(b),
        Some((_, b)) => Err(tag(&b, "errorDescription").or_else(|| tag(&b, "errorCode")).unwrap_or("refused").to_string()),
        None => Err("no answer".into()),
    }
}

fn add_mapping(g: &Gateway, external: u16, lease: u32) -> Result<(), String> {
    soap(
        g,
        "AddPortMapping",
        &format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{external}</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>{TCP_PORT}</NewInternalPort><NewInternalClient>{}</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>Oxide Remote</NewPortMappingDescription><NewLeaseDuration>{lease}</NewLeaseDuration>",
            g.local_ip
        ),
    )
    .map(|_| ())
}

fn delete_mapping(g: &Gateway, external: u16) {
    let _ = soap(g, "DeletePortMapping", &format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>{external}</NewExternalPort><NewProtocol>TCP</NewProtocol>"));
}

fn external_ip(g: &Gateway) -> Option<Ipv4Addr> {
    let b = soap(g, "GetExternalIPAddress", "").ok()?;
    let ip: Ipv4Addr = tag(&b, "NewExternalIPAddress")?.parse().ok()?;
    // a private "public" address means another router (or the provider) is in front: not reachable that way
    (!ip.is_private() && !ip.is_unspecified() && !(ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64)).then_some(ip)
}

/// Forward a port; tries a few in case another device uses the first. Lease first, permanent if the router
/// only takes those.
fn map_port(g: &Gateway, prefer: Option<u16>) -> Result<u16, String> {
    let mut last = String::new();
    let tries: Vec<u16> = prefer.into_iter().chain([TCP_PORT]).chain(47830..47836).collect();
    for p in tries {
        match add_mapping(g, p, 3600) {
            Ok(()) => return Ok(p),
            Err(e) if e.contains("OnlyPermanentLeasesSupported") || e.contains("725") => {
                if add_mapping(g, p, 0).is_ok() {
                    return Ok(p);
                }
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

// ------------------------------------------------------------- this PC's other addresses

fn ipv6_global() -> Vec<String> {
    // /proc/net/if_inet6: address, index, prefix, scope, flags, name; scope 00 = global, flag 0x01 = temporary
    let Ok(t) = std::fs::read_to_string("/proc/net/if_inet6") else { return vec![] };
    let mut v = vec![];
    for line in t.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 6 || f[3] != "00" || f[5] == "lo" {
            continue;
        }
        let flags = u32::from_str_radix(f[4], 16).unwrap_or(0);
        if flags & 0x01 != 0 {
            continue; // temporary (privacy) addresses change often
        }
        let Some(b) = unhex(f[0]) else { continue };
        let Ok(b): Result<[u8; 16], _> = b.try_into() else { continue };
        let ip = std::net::Ipv6Addr::from(b);
        if (ip.segments()[0] & 0xfe00) == 0xfc00 && !f[5].starts_with("tailscale") {
            continue; // unique-local, except a VPN's
        }
        v.push(format!("[{ip}]:{TCP_PORT}"));
    }
    v
}

/// This PC's addresses on its local networks (not VPNs, containers or virtual machines).
fn lan_v4() -> Vec<String> {
    let Ok(out) = std::process::Command::new("ip").args(["-4", "-o", "addr", "show", "scope", "global"]).output() else { return vec![] };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let dev = w.nth(1)?;
            if ["docker", "br-", "virbr", "veth", "tailscale", "zt", "tun", "wg", "lo"].iter().any(|p| dev.starts_with(p)) {
                return None;
            }
            let a = l.split_whitespace().skip_while(|x| *x != "inet").nth(1)?;
            let ip: Ipv4Addr = a.split('/').next()?.parse().ok()?;
            ip.is_private().then(|| format!("{ip}:{TCP_PORT}"))
        })
        .collect()
}

fn vpn_v4() -> Vec<String> {
    // the addresses of VPN interfaces (Tailscale, ZeroTier use 100.64.0.0/10)
    let Ok(out) = std::process::Command::new("ip").args(["-4", "-o", "addr", "show"]).output() else { return vec![] };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let a = l.split_whitespace().skip_while(|w| *w != "inet").nth(1)?;
            let ip: Ipv4Addr = a.split('/').next()?.parse().ok()?;
            (ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64).then(|| format!("{ip}:{TCP_PORT}"))
        })
        .collect()
}

// ------------------------------------------------------------- notes for the phones

fn post_notes(server: &str, store: &Store, w: &Whereabouts) {
    let plain = serde_json::to_vec(w).unwrap_or_default();
    for p in &store.phones {
        let Some(ltk) = unhex(&p.ltk).and_then(|v| <[u8; 32]>::try_from(v).ok()) else { continue };
        let url = format!("{}/{}", server.trim_end_matches('/'), note_topic(&ltk));
        let r = ureq::post(&url).timeout(Duration::from_secs(10)).set("Cache", "yes").send_string(&note_seal(&ltk, &plain));
        if let Err(e) = r {
            eprintln!("oxide-remoted: could not leave a note for {}: {e}", p.name);
        }
    }
}

/// Keeps `reach` up to date while the option is on (checked every minute; the router every half hour).
pub fn start(reach: Arc<Mutex<Reach>>, store: Arc<Mutex<Store>>) {
    std::thread::spawn(move || {
        let mut gateway: Option<Gateway> = None;
        let mut mapped: Option<u16> = None;
        let mut last_router = std::time::Instant::now() - Duration::from_secs(3600);
        let mut last_note: (Vec<String>, u64) = (vec![], 0);
        let mut phones = 0usize;
        loop {
            let cfg = oxide_config::Config::load().unwrap_or_default().remote;
            let mut r = Reach { on: cfg.anywhere, rendezvous: cfg.rendezvous.clone(), ..Default::default() };
            if !cfg.anywhere {
                if let (Some(g), Some(p)) = (&gateway, mapped.take()) {
                    delete_mapping(g, p);
                }
            } else {
                if last_router.elapsed() > Duration::from_secs(1800) || mapped.is_none() {
                    last_router = std::time::Instant::now();
                    if gateway.is_none() {
                        gateway = discover_gateway();
                    }
                    match &gateway {
                        Some(g) => match map_port(g, mapped) {
                            Ok(p) => mapped = Some(p),
                            Err(e) => {
                                mapped = None;
                                r.problem = Some(format!("The router did not forward the port ({e})."));
                            }
                        },
                        None => r.problem = Some("No router answered (UPnP is off, or this network has none).".into()),
                    }
                } else {
                    let prev = reach.lock().unwrap().clone();
                    r.problem = prev.problem;
                }
                if let (Some(g), Some(p)) = (&gateway, mapped) {
                    match external_ip(g) {
                        Some(ip) => {
                            r.public_ip = Some(ip.to_string());
                            r.upnp_port = Some(p);
                            r.addrs.push(format!("{ip}:{p}"));
                        }
                        None if r.problem.is_none() => {
                            r.problem = Some("The internet provider gives this network no public IPv4 address (CGNAT). IPv6 or a VPN like Tailscale can still work.".into())
                        }
                        None => {}
                    }
                }
                r.addrs.extend(ipv6_global());
            }
            r.addrs.extend(vpn_v4());
            r.lan = lan_v4();
            *reach.lock().unwrap() = r.clone();
            // a note when the addresses change (another network), for a new phone, and twice a day (ntfy keeps
            // notes for 12 hours)
            let st = store.lock().map(|s| s.clone()).unwrap_or_default();
            let mut sorted: Vec<String> = r.addrs.iter().chain(&r.lan).cloned().collect();
            sorted.sort();
            let due = sorted != last_note.0 || st.phones.len() != phones || store::now() - last_note.1 > 6 * 3600;
            if cfg.notes && !st.phones.is_empty() && !sorted.is_empty() && due {
                post_notes(&cfg.rendezvous, &st, &r.whereabouts());
                last_note = (sorted, store::now());
                phones = st.phones.len();
            }
            std::thread::sleep(Duration::from_secs(if cfg.anywhere { 60 } else { 30 }));
        }
    });
}

/// Remove the port forwarding when the daemon stops.
pub fn stop(reach: &Arc<Mutex<Reach>>) {
    let r = reach.lock().unwrap().clone();
    if let Some(p) = r.upnp_port {
        if let Some(g) = discover_gateway() {
            delete_mapping(&g, p);
        }
    }
}
