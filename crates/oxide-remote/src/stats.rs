//! System numbers for the phone: CPU (total and per core), memory, disks, temperatures, network, battery,
//! the busiest processes, volume and what is playing. Read from /proc and /sys; sampled once a second, and only
//! while a phone is looking.
use std::{
    collections::HashMap,
    sync::{atomic::{AtomicU32, Ordering}, Arc, Mutex},
    time::Duration,
};

use oxide_remote_proto::{Disk, Proc, Stats, Temp};

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_default()
}

fn cpu_lines() -> Vec<(u64, u64)> {
    read("/proc/stat")
        .lines()
        .filter(|l| l.starts_with("cpu"))
        .map(|l| {
            let v: Vec<u64> = l.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
            let idle = v.get(3).copied().unwrap_or(0) + v.get(4).copied().unwrap_or(0);
            (v.iter().sum(), idle)
        })
        .collect()
}

fn meminfo() -> HashMap<String, u64> {
    read("/proc/meminfo")
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let k = it.next()?.trim_end_matches(':').to_string();
            let v: u64 = it.next()?.parse().ok()?;
            Some((k, v * 1024))
        })
        .collect()
}

fn disks() -> Vec<Disk> {
    let mut out: Vec<Disk> = vec![];
    for l in read("/proc/mounts").lines() {
        let f: Vec<&str> = l.split_whitespace().collect();
        if f.len() < 3 || !f[0].starts_with("/dev/") {
            continue;
        }
        if !matches!(f[2], "ext4" | "ext3" | "btrfs" | "xfs" | "f2fs" | "vfat" | "exfat" | "ntfs" | "ntfs3" | "zfs" | "bcachefs") {
            continue;
        }
        let mount = f[1].replace("\\040", " ");
        if mount.starts_with("/snap") || mount.starts_with("/var/lib") {
            continue;
        }
        if let Ok(s) = rustix::fs::statvfs(mount.as_str()) {
            let total = s.f_blocks * s.f_frsize;
            let used = total.saturating_sub(s.f_bavail * s.f_frsize);
            // btrfs subvolumes repeat the same filesystem: keep one
            if total > 0 && !out.iter().any(|d| d.total == total && d.used == used) {
                out.push(Disk { mount, used, total });
            }
        }
    }
    out
}

fn temps() -> Vec<Temp> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir("/sys/class/hwmon") else { return out };
    for e in rd.flatten() {
        let dir = e.path();
        let chip = read(&dir.join("name").to_string_lossy()).trim().to_string();
        let nice = match chip.as_str() {
            "coretemp" | "k10temp" | "zenpower" | "cpu_thermal" => "CPU",
            "amdgpu" | "nouveau" | "radeon" | "i915" | "xe" => "GPU",
            "nvme" => "SSD",
            "acpitz" => "Board",
            _ => continue,
        };
        // the first (package / composite / edge) sensor of each chip
        let t = (1..=8).find_map(|i| read(&dir.join(format!("temp{i}_input")).to_string_lossy()).trim().parse::<f32>().ok());
        if let Some(t) = t {
            if !out.iter().any(|x: &Temp| x.label == nice) {
                out.push(Temp { label: nice.into(), celsius: t / 1000.0 });
            }
        }
    }
    out
}

fn net_totals() -> (u64, u64) {
    let mut rx = 0;
    let mut tx = 0;
    for l in read("/proc/net/dev").lines().skip(2) {
        let Some((name, rest)) = l.split_once(':') else { continue };
        let name = name.trim();
        if name == "lo" || name.starts_with("veth") || name.starts_with("docker") || name.starts_with("br-") {
            continue;
        }
        let v: Vec<u64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        rx += v.first().copied().unwrap_or(0);
        tx += v.get(8).copied().unwrap_or(0);
    }
    (rx, tx)
}

fn battery() -> (Option<f32>, bool) {
    let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") else { return (None, false) };
    for e in rd.flatten() {
        let p = e.path();
        if read(&p.join("type").to_string_lossy()).trim() != "Battery" {
            continue;
        }
        let cap = read(&p.join("capacity").to_string_lossy()).trim().parse::<f32>().ok();
        let charging = read(&p.join("status").to_string_lossy()).trim() == "Charging";
        return (cap, charging);
    }
    (None, false)
}

fn gpu_busy() -> Option<f32> {
    let rd = std::fs::read_dir("/sys/class/drm").ok()?;
    for e in rd.flatten() {
        if let Ok(v) = read(&e.path().join("device/gpu_busy_percent").to_string_lossy()).trim().parse::<f32>() {
            return Some(v / 100.0);
        }
    }
    None
}

fn proc_times() -> HashMap<u32, (String, u64, u64)> {
    let mut m = HashMap::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return m };
    let page = 4096u64;
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else { continue };
        let stat = read(&format!("/proc/{pid}/stat"));
        let (Some(o), Some(c)) = (stat.find('('), stat.rfind(')')) else { continue };
        let name = stat[o + 1..c].to_string();
        let f: Vec<&str> = stat[c + 1..].split_whitespace().collect();
        let t = f.get(11).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0) + f.get(12).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
        let rss = f.get(21).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0) * page;
        m.insert(pid, (name, t, rss));
    }
    m
}

fn volume() -> (Option<f32>, bool) {
    let out = std::process::Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output();
    let Ok(o) = out else { return (None, false) };
    let t = String::from_utf8_lossy(&o.stdout).to_string();
    let v = t.split_whitespace().nth(1).and_then(|x| x.parse::<f32>().ok());
    (v, t.contains("MUTED"))
}

fn media() -> Option<String> {
    let o = std::process::Command::new("playerctl").args(["metadata", "--format", "{{artist}} - {{title}}"]).output().ok()?;
    let s = String::from_utf8_lossy(&o.stdout).trim().trim_start_matches(" - ").to_string();
    (!s.is_empty() && o.status.success()).then_some(s)
}

fn workspace() -> Option<u32> {
    match oxide_ipc::call(&oxide_ipc::Request::GetState).ok()? {
        oxide_ipc::Response::State { state } => Some(state.active_workspace),
        _ => None,
    }
}

/// The latest numbers, and how many phones want them.
#[derive(Clone, Default)]
pub struct Sampler {
    pub latest: Arc<Mutex<Stats>>,
    pub watchers: Arc<AtomicU32>,
}

impl Sampler {
    pub fn start(&self) {
        let s = self.clone();
        std::thread::spawn(move || {
            let mut prev_cpu = cpu_lines();
            let mut prev_net = net_totals();
            let mut prev_proc = proc_times();
            let mut tick = 0u64;
            loop {
                std::thread::sleep(Duration::from_secs(1));
                if s.watchers.load(Ordering::Relaxed) == 0 {
                    prev_cpu = cpu_lines();
                    prev_net = net_totals();
                    prev_proc = proc_times();
                    continue;
                }
                tick += 1;
                let cpu = cpu_lines();
                let usage: Vec<f32> = cpu
                    .iter()
                    .zip(prev_cpu.iter())
                    .map(|((t1, i1), (t0, i0))| {
                        let dt = t1.saturating_sub(*t0);
                        if dt == 0 { 0.0 } else { 1.0 - i1.saturating_sub(*i0) as f32 / dt as f32 }
                    })
                    .collect();
                let total_dt = cpu.first().zip(prev_cpu.first()).map(|(a, b)| a.0.saturating_sub(b.0)).unwrap_or(1).max(1);
                prev_cpu = cpu;
                let net = net_totals();
                let (rx, tx) = (net.0.saturating_sub(prev_net.0), net.1.saturating_sub(prev_net.1));
                prev_net = net;
                let procs = proc_times();
                let ncores = usage.len().saturating_sub(1).max(1) as f32;
                let mut top: Vec<Proc> = procs
                    .iter()
                    .filter_map(|(pid, (name, t, rss))| {
                        let t0 = prev_proc.get(pid).map(|p| p.1).unwrap_or(*t);
                        let cpu = t.saturating_sub(t0) as f32 / total_dt as f32 * ncores;
                        (cpu > 0.0 || *rss > 200 << 20).then(|| Proc { name: name.clone(), cpu, mem: *rss })
                    })
                    .collect();
                top.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then(b.mem.cmp(&a.mem)));
                top.truncate(8);
                prev_proc = procs;
                let mem = meminfo();
                let g = |k: &str| mem.get(k).copied().unwrap_or(0);
                let load: Vec<f32> = read("/proc/loadavg").split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
                let uptime = read("/proc/uptime").split_whitespace().next().and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0) as u64;
                let (battery, charging) = battery();
                let mut st = s.latest.lock().map(|g| g.clone()).unwrap_or_default();
                st.cpu = usage.first().copied().unwrap_or(0.0);
                st.cores = usage.iter().skip(1).copied().collect();
                st.load = [load.first().copied().unwrap_or(0.0), load.get(1).copied().unwrap_or(0.0), load.get(2).copied().unwrap_or(0.0)];
                st.mem_total = g("MemTotal");
                st.mem_used = g("MemTotal").saturating_sub(g("MemAvailable"));
                st.swap_total = g("SwapTotal");
                st.swap_used = g("SwapTotal").saturating_sub(g("SwapFree"));
                st.net_rx = rx;
                st.net_tx = tx;
                st.top = top;
                st.uptime = uptime;
                st.battery = battery;
                st.charging = charging;
                st.gpu = gpu_busy();
                // the slower ones every few seconds
                if tick % 3 == 1 {
                    st.disks = disks();
                    st.temps = temps();
                    let (v, m) = volume();
                    st.volume = v;
                    st.muted = m;
                    st.media = media();
                    st.workspace = workspace();
                }
                if let Ok(mut l) = s.latest.lock() {
                    *l = st;
                }
            }
        });
    }
}
