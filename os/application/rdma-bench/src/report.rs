//! Result types and table formatting shared by the single-run and the suite path.
//!
//! The benchmarks themselves only compute numbers and hand back a `Report`; all printing happens
//! here, so a suite can emit one header followed by a row per message size instead of repeating a
//! full table for every run.

use crate::cli::Mode;
use crate::comm::AccuracyReport;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

pub struct BandwidthStats {
    pub msg_size: usize,
    pub iterations: usize,
    pub tx_depth: usize,
    pub elapsed_us: usize,
}

impl BandwidthStats {
    fn secs(&self) -> f64 {
        self.elapsed_us as f64 / 1_000_000.0
    }

    pub fn bw_gbps(&self) -> f64 {
        self.iterations as f64 * self.msg_size as f64 * 8.0 / self.secs() / 1e9
    }

    pub fn msg_rate_mpps(&self) -> f64 {
        self.iterations as f64 / self.secs() / 1e6
    }
}

pub struct LatencyStats {
    pub msg_size: usize,
    pub samples: usize,
    pub min: f64,
    pub max: f64,
    pub typical: f64,
    pub avg: f64,
    pub stdev: f64,
    pub p99: f64,
    pub p999: f64,
}

impl LatencyStats {
    /// Nearest-rank percentiles over the (half round trip, microsecond) samples, matching
    /// `perftest`'s `ib_send_lat` layout. `None` when nothing was measured at all.
    pub fn from_samples(msg_size: usize, samples: &[f64]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }

        let mut sorted: Vec<f64> = samples.to_vec();
        sorted.sort_by(f64::total_cmp);

        let n = sorted.len();
        let avg = sorted.iter().sum::<f64>() / n as f64;
        let stdev = if n > 1 {
            libm::sqrt(sorted.iter().map(|s| libm::pow(s - avg, 2f64)).sum::<f64>() / (n - 1) as f64)
        } else {
            0.0
        };
        let percentile = |p: f64| sorted[(libm::ceil((n as f64) * p) as usize).clamp(1, n) - 1];

        Some(LatencyStats {
            msg_size,
            samples: n,
            min: sorted[0],
            max: sorted[n - 1],
            typical: percentile(0.50),
            avg,
            stdev,
            p99: percentile(0.99),
            p999: percentile(0.999),
        })
    }
}

pub enum Report {
    Bandwidth(BandwidthStats),
    /// `None` when every round trip timed out before a sample could be taken.
    Latency(Option<LatencyStats>),
    Accuracy(AccuracyReport),
    /// The passive side of a run — it has no numbers of its own to show.
    Peer,
}

impl Report {
    /// The row this report contributes to its mode's table, or `None` for a run that produced no
    /// numbers.
    pub fn row(&self) -> Option<String> {
        match self {
            Report::Bandwidth(s) => Some(format!(
                "{:>8}  {:>12}  {:>10}  {:>18.6}  {:>14.6}",
                s.msg_size,
                s.iterations,
                s.tx_depth,
                s.bw_gbps(),
                s.msg_rate_mpps()
            )),
            Report::Latency(Some(s)) => Some(format!(
                "{:>8}  {:>12}  {:>12.2}  {:>12.2}  {:>16.2}  {:>12.2}  {:>14.2}  {:>10.2}  {:>12.2}",
                s.msg_size, s.samples, s.min, s.max, s.typical, s.avg, s.stdev, s.p99, s.p999
            )),
            Report::Latency(None) => None,
            Report::Accuracy(r) => {
                let total_bytes = (r.sent * r.msg_size) as f64;
                let byte_acc = r.correct_bytes as f64 / total_bytes * 100.0;
                let bit_acc = r.correct_bits as f64 / (total_bytes * 8.0) * 100.0;
                Some(format!(
                    "{:>8}  {:>12}  {:>10}  {:>8}  {:>6}  {:>10}  {:>12.4}  {:>12.4}",
                    r.msg_size, r.sent, r.received, r.lost, r.duplicated, r.corrupted, byte_acc, bit_acc
                ))
            }
            Report::Peer => None,
        }
    }

    /// Extra lines worth showing below the row (never part of the table itself).
    pub fn notes(&self) -> Option<String> {
        match self {
            Report::Accuracy(r) if r.unidentifiable > 0 || r.truncated > 0 => {
                Some(format!("unidentifiable: {}, truncated: {}", r.unidentifiable, r.truncated))
            }
            Report::Latency(None) => Some("no samples collected".into()),
            _ => None,
        }
    }

    /// Prints this single report as a standalone table (header + row).
    pub fn print(&self, mode: Mode) {
        if let Some(row) = self.row() {
            terminal::println!("{}", header(mode));
            terminal::println!("{}", row);
        }
        if let Some(notes) = self.notes() {
            terminal::println!("{}", notes);
        }
    }
}

pub fn header(mode: Mode) -> String {
    match mode {
        Mode::Bandwidth => format!(
            "{:>8}  {:>12}  {:>10}  {:>18}  {:>14}",
            "#bytes", "#iterations", "tx_depth", "BW avg[Gb/sec]", "MsgRate[Mpps]"
        ),
        Mode::Latency => format!(
            "{:>8}  {:>12}  {:>12}  {:>12}  {:>16}  {:>12}  {:>14}  {:>10}  {:>12}",
            "#bytes",
            "#iterations",
            "t_min[usec]",
            "t_max[usec]",
            "t_typical[usec]",
            "t_avg[usec]",
            "t_stdev[usec]",
            "99%[usec]",
            "99.9%[usec]"
        ),
        Mode::Accuracy => format!(
            "{:>8}  {:>12}  {:>10}  {:>8}  {:>6}  {:>10}  {:>12}  {:>12}",
            "#bytes", "#iterations", "#received", "#lost", "#dup", "#corrupt", "ByteAcc[%]", "BitAcc[%]"
        ),
    }
}
