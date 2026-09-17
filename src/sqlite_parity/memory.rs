use std::fs;

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessMemory {
    pub peak_rss_kb: Option<u64>,
    pub rss_sampled_kb: Option<u64>,
}

impl ProcessMemory {
    pub fn observe_pid(&mut self, pid: u32) {
        let Some(sample) = read_linux_status(pid) else {
            return;
        };
        self.rss_sampled_kb = max_option(self.rss_sampled_kb, sample.rss_sampled_kb);
        self.peak_rss_kb = max_option(self.peak_rss_kb, sample.peak_rss_kb);
    }

    pub fn status(self, enabled: bool) -> &'static str {
        if !enabled {
            "disabled"
        } else if self.peak_rss_kb.is_some() || self.rss_sampled_kb.is_some() {
            "sampled"
        } else {
            "unavailable"
        }
    }
}

fn read_linux_status(pid: u32) -> Option<ProcessMemory> {
    let text = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let mut sample = ProcessMemory::default();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("VmHWM:") {
            sample.peak_rss_kb = parse_status_kb(value);
        } else if let Some(value) = line.strip_prefix("VmRSS:") {
            sample.rss_sampled_kb = parse_status_kb(value);
        }
    }
    Some(sample)
}

fn parse_status_kb(value: &str) -> Option<u64> {
    value.split_whitespace().next()?.parse().ok()
}

fn max_option(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}
