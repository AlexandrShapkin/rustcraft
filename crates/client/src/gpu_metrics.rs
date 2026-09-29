//! Optional OS telemetry, separate from renderer contracts. Read-only sysfs;
//! match exactly one physical device or report unavailable rather than guessing.
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
#[derive(Default, Debug)]
pub struct Snapshot {
    pub utilization: Option<f64>,
    pub used_mib: Option<f64>,
    pub total_mib: Option<f64>,
}
pub struct Provider {
    path: Option<PathBuf>,
    last: Instant,
    pub snapshot: Snapshot,
}
impl Provider {
    pub fn new(vendor: u32, device: u32) -> Self {
        let mut matches = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for e in entries.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if !name
                    .strip_prefix("card")
                    .is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
                {
                    continue;
                }
                let path = e.path().join("device");
                let hex = |name| {
                    std::fs::read_to_string(path.join(name)).ok().and_then(|s| {
                        u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
                    })
                };
                if hex("vendor") == Some(vendor) && hex("device") == Some(device) {
                    matches.push(path);
                }
            }
        }
        Self {
            path: if matches.len() == 1 {
                matches.pop()
            } else {
                None
            },
            last: Instant::now() - Duration::from_secs(1),
            snapshot: Snapshot::default(),
        }
    }
    pub fn sample(&mut self) {
        if self.last.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last = Instant::now();
        let Some(path) = &self.path else {
            return;
        };
        let read = |name| {
            std::fs::read_to_string(path.join(name))
                .ok()
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v >= 0.)
        };
        self.snapshot.utilization = read("gpu_busy_percent").filter(|v| *v <= 100.);
        self.snapshot.used_mib = read("mem_info_vram_used").map(|v| v / 1048576.);
        self.snapshot.total_mib = read("mem_info_vram_total").map(|v| v / 1048576.);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_adapter_stays_unavailable() {
        let mut provider = Provider::new(u32::MAX, u32::MAX);
        provider.sample();
        assert!(provider.snapshot.utilization.is_none());
        assert!(provider.snapshot.used_mib.is_none());
        assert!(provider.snapshot.total_mib.is_none());
    }
}
