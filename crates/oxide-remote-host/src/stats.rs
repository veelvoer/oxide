//! The numbers the phone shows: processor, memory, disks, network, battery, the busiest programs.
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use oxide_remote_proto::{Disk, Proc, Stats, Temp};
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};

#[derive(Clone, Default)]
pub struct Sampler {
    pub latest: Arc<Mutex<Stats>>,
    /// phones that look at the numbers now; nothing is measured while nobody looks
    pub watchers: Arc<AtomicUsize>,
}

impl Sampler {
    pub fn start(&self) {
        let me = self.clone();
        std::thread::spawn(move || {
            let (Ok(mut sys), Ok(mut disks), Ok(mut nets), Ok(mut comps)) =
                (System::new(), Disks::new_with_refreshed_list(), Networks::new_with_refreshed_list(), Components::new_with_refreshed_list())
            else {
                eprintln!("oxide-remote-host: cannot read system information");
                return;
            };
            let mut tick = 0u32;
            loop {
                if me.watchers.load(Ordering::Relaxed) == 0 {
                    std::thread::sleep(Duration::from_millis(400));
                    continue;
                }
                sys.refresh_cpu_usage();
                sys.refresh_memory();
                sys.refresh_processes(ProcessesToUpdate::All, true);
                nets.refresh(true);
                if tick % 10 == 0 {
                    disks.refresh(true);
                    comps.refresh(true);
                }
                tick = tick.wrapping_add(1);
                let (rx, tx) = nets.iter().fold((0u64, 0u64), |a, (_, d)| (a.0 + d.received(), a.1 + d.transmitted()));
                let mut top: Vec<Proc> = sys
                    .processes()
                    .values()
                    .map(|p| Proc { name: p.name().to_string_lossy().to_string(), cpu: p.cpu_usage() / 100.0 / sys.cpus().len().max(1) as f32, mem: p.memory() })
                    .collect();
                top.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
                top.truncate(5);
                let (battery, charging) = crate::platform::battery();
                let s = Stats {
                    cpu: sys.global_cpu_usage() / 100.0,
                    cores: sys.cpus().iter().map(|c| c.usage() / 100.0).collect(),
                    load: [0.0; 3],
                    mem_used: sys.used_memory(),
                    mem_total: sys.total_memory(),
                    swap_used: sys.used_swap(),
                    swap_total: sys.total_swap(),
                    disks: disks
                        .iter()
                        .filter(|d| d.total_space() > 0)
                        .map(|d| Disk { mount: d.mount_point().to_string_lossy().to_string(), total: d.total_space(), used: d.total_space() - d.available_space() })
                        .collect(),
                    temps: comps.iter().filter_map(|c| c.temperature().map(|t| Temp { label: c.label().to_string(), celsius: t })).take(4).collect(),
                    net_rx: rx,
                    net_tx: tx,
                    battery,
                    charging,
                    uptime: System::uptime().unwrap_or(0),
                    top,
                    gpu: None,
                    volume: None,
                    muted: false,
                    media: None,
                    workspace: None,
                };
                if let Ok(mut l) = me.latest.lock() {
                    *l = s;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    }
}
