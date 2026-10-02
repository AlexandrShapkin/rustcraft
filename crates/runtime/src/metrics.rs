//! Headless counters. 1% low = reciprocal mean of slowest ceil(N/100) frames,
//! unavailable until 100 samples. CPU 100% means one fully occupied logical CPU.
use std::time::Instant;
pub struct History {
    samples: [f64; 240],
    cursor: usize,
    len: usize,
}
impl Default for History {
    fn default() -> Self {
        Self {
            samples: [0.; 240],
            cursor: 0,
            len: 0,
        }
    }
}
impl History {
    pub fn push(&mut self, seconds: f64) {
        if seconds.is_finite() && seconds > 0. {
            self.samples[self.cursor] = seconds;
            self.cursor = (self.cursor + 1) % 240;
            self.len = (self.len + 1).min(240);
        }
    }
    pub fn last(&self) -> Option<f64> {
        (self.len > 0).then(|| self.samples[(self.cursor + 239) % 240])
    }
    pub fn mean(&self) -> Option<f64> {
        (self.len > 0).then(|| self.samples[..self.len].iter().sum::<f64>() / self.len as f64)
    }
    pub fn fps(&self) -> Option<f64> {
        self.mean().map(|s| 1. / s)
    }
    pub fn low(&self) -> Option<f64> {
        if self.len < 100 {
            return None;
        }
        let mut scratch = self.samples;
        scratch[..self.len].sort_unstable_by(|a, b| b.total_cmp(a));
        let n = self.len.div_ceil(100);
        Some(n as f64 / scratch[..n].iter().sum::<f64>())
    }
}
pub struct Metrics {
    pub frames: History,
    pub ticks: History,
    pub meshes: History,
    pub tps: Option<f64>,
    pub rebuilds_per_second: Option<f64>,
    meshes_in_epoch: u32,
    pub steps: u32,
    pub catch_up: bool,
    epoch: Instant,
    ticks_in_epoch: u32,
    pub uptime: Instant,
}
impl Default for Metrics {
    fn default() -> Self {
        Self {
            frames: History::default(),
            ticks: History::default(),
            meshes: History::default(),
            tps: None,
            rebuilds_per_second: None,
            meshes_in_epoch: 0,
            steps: 0,
            catch_up: false,
            epoch: Instant::now(),
            ticks_in_epoch: 0,
            uptime: Instant::now(),
        }
    }
}
impl Metrics {
    pub fn tick(&mut self, seconds: f64) {
        self.ticks.push(seconds);
        self.ticks_in_epoch += 1;
    }
    pub fn mesh(&mut self, seconds: f64) {
        self.meshes.push(seconds);
        self.meshes_in_epoch += 1;
    }
    pub fn update(&mut self) {
        self.update_at(Instant::now());
    }
    pub fn update_at(&mut self, now: Instant) {
        let elapsed = now.duration_since(self.epoch).as_secs_f64();
        if elapsed >= 1. {
            self.tps = Some(self.ticks_in_epoch as f64 / elapsed);
            self.rebuilds_per_second = Some(self.meshes_in_epoch as f64 / elapsed);
            self.ticks_in_epoch = 0;
            self.meshes_in_epoch = 0;
            self.epoch = now;
        }
    }
}
#[derive(Default, Debug, Clone)]
pub struct ProcessSnapshot {
    pub rss_mib: Option<f64>,
    pub cpu: Option<f64>,
    pub threads: Option<usize>,
    pub logical_cpus: Option<usize>,
}
pub struct ProcessSampler {
    last: Instant,
    #[cfg(target_os = "linux")]
    previous: Option<(u64, u64)>,
    pub snapshot: ProcessSnapshot,
}
impl Default for ProcessSampler {
    fn default() -> Self {
        Self {
            last: Instant::now() - std::time::Duration::from_secs(1),
            #[cfg(target_os = "linux")]
            previous: None,
            snapshot: ProcessSnapshot::default(),
        }
    }
}
impl ProcessSampler {
    pub fn sample(&mut self) {
        self.sample_at(Instant::now());
    }
    pub fn sample_at(&mut self, now: Instant) -> bool {
        if now.duration_since(self.last).as_millis() < 500 {
            return false;
        }
        self.last = now;
        self.snapshot = ProcessSnapshot::default();
        #[cfg(target_os = "linux")]
        {
            self.linux_sample();
        }
        true
    }
    #[cfg(target_os = "linux")]
    fn linux_sample(&mut self) {
        let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
            return;
        };
        for line in status.lines() {
            let mut words = line.split_whitespace();
            match words.next() {
                Some("VmRSS:") => {
                    self.snapshot.rss_mib = words
                        .next()
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|v| v / 1024.)
                }
                Some("Threads:") => {
                    self.snapshot.threads = words.next().and_then(|s| s.parse().ok())
                }
                _ => {}
            }
        }
        let counters = (|| {
            let process = std::fs::read_to_string("/proc/self/stat").ok()?;
            let fields: Vec<_> = process.rsplit_once(')')?.1.split_whitespace().collect();
            let used =
                fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
            let system = std::fs::read_to_string("/proc/stat").ok()?;
            let total = system
                .lines()
                .next()?
                .split_whitespace()
                .skip(1)
                .take(8)
                .map(str::parse::<u64>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?
                .iter()
                .sum();
            let cpus = system
                .lines()
                .filter(|s| {
                    s.starts_with("cpu") && s.as_bytes().get(3).is_some_and(u8::is_ascii_digit)
                })
                .count();
            Some((used, total, cpus))
        })();
        if let Some((used, total, cpus)) = counters {
            self.snapshot.logical_cpus = Some(cpus);
            if let Some((old_used, old_total)) = self.previous
                && total > old_total
            {
                self.snapshot.cpu = Some(
                    used.saturating_sub(old_used) as f64 / (total - old_total) as f64
                        * cpus as f64
                        * 100.,
                );
            }
            self.previous = Some((used, total));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_low_is_not_faked() {
        let mut h = History::default();
        for _ in 0..99 {
            h.push(0.01);
        }
        assert_eq!(h.low(), None);
        h.push(0.1);
        assert!((h.low().unwrap() - 10.).abs() < 1e-6);
        for _ in 0..240 {
            h.push(0.02);
        }
        assert_eq!(h.len, 240);
        assert!((h.fps().unwrap() - 50.).abs() < 1e-6);
    }
    #[test]
    fn unsupported_values_are_absent() {
        let p = ProcessSnapshot::default();
        assert!(p.cpu.is_none() && p.rss_mib.is_none());
    }
}

/// 20 Hz, at most five ticks per event-loop turn. Drop whole excess ticks while
/// retaining the fractional remainder; later ticks always retain their 50 ms dt.
#[derive(Default)]
pub struct FixedStepClock {
    remainder: f64,
}
#[derive(Debug)]
pub struct StepBudget {
    pub steps: u32,
    pub catch_up: bool,
    pub dropped_seconds: f64,
}
impl FixedStepClock {
    pub const DT: f64 = 0.05;
    pub fn advance(&mut self, elapsed: f64) -> StepBudget {
        if elapsed.is_finite() && elapsed > 0. {
            self.remainder += elapsed;
        }
        let due = (self.remainder / Self::DT).floor();
        let steps = due.min(5.) as u32;
        self.remainder -= due * Self::DT;
        StepBudget {
            steps,
            catch_up: due > 1.,
            dropped_seconds: (due - f64::from(steps)) * Self::DT,
        }
    }
    #[must_use]
    pub fn alpha(&self) -> f32 {
        (self.remainder / Self::DT).clamp(0.0, 1.0) as f32
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn actual_rates_use_elapsed_wall_time() {
        let mut m = Metrics::default();
        for _ in 0..17 {
            m.tick(0.0002);
        }
        m.mesh(0.002);
        let now = m.epoch + Duration::from_secs(2);
        m.update_at(now);
        assert_eq!(m.tps, Some(8.5));
        assert_eq!(m.rebuilds_per_second, Some(0.5));
        assert_eq!(m.ticks.last(), Some(0.0002));
        m.update_at(now + Duration::from_secs(1));
        assert_eq!(m.tps, Some(0.));
    }
    #[test]
    fn long_pause_is_bounded_and_normal_speed_resumes() {
        let mut clock = FixedStepClock::default();
        let b = clock.advance(2.02);
        assert_eq!(b.steps, 5);
        assert!(b.catch_up);
        assert!((b.dropped_seconds - 1.75).abs() < 1e-9);
        assert_eq!(clock.advance(0.031).steps, 1);
        assert_eq!(clock.advance(0.05).steps, 1);
        assert_eq!(clock.advance(f64::NAN).steps, 0);
    }
    #[test]
    fn process_sampling_is_periodic() {
        let mut p = ProcessSampler::default();
        let now = Instant::now();
        assert!(p.sample_at(now));
        assert!(!p.sample_at(now + Duration::from_millis(499)));
        assert!(p.sample_at(now + Duration::from_millis(500)));
    }
    #[test]
    fn invalid_samples_do_not_pollute_history() {
        let mut h = History::default();
        for v in [0., -1., f64::NAN, f64::INFINITY] {
            h.push(v);
        }
        assert_eq!(h.fps(), None);
        for _ in 0..200 {
            h.push(0.01);
        }
        h.push(0.04);
        h.push(0.08);
        // ceil(202/100) = 3: mean of .08, .04, .01.
        assert!((h.low().unwrap() - 3. / 0.13).abs() < 1e-9);
    }
}
