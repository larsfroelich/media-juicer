#[derive(Debug, Clone, PartialEq)]
pub struct ProgressSnapshot {
    pub processed_files: usize,
    pub total_files: usize,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub processed_mb: f64,
    pub total_mb: f64,
    pub percent_complete: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressTracker {
    total_files: usize,
    total_bytes: u64,
    processed_files: usize,
    processed_bytes: u64,
}

impl ProgressTracker {
    pub fn new(total_files: usize, total_bytes: u64) -> Self {
        Self {
            total_files,
            total_bytes,
            processed_files: 0,
            processed_bytes: 0,
        }
    }

    pub fn record_processed(&mut self, file_size: u64) {
        self.processed_files += 1;
        self.processed_bytes += file_size;
    }

    pub fn percent_complete(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }

        round_to((self.processed_bytes as f64 / self.total_bytes as f64) * 100.0, 2)
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            processed_files: self.processed_files,
            total_files: self.total_files,
            processed_bytes: self.processed_bytes,
            total_bytes: self.total_bytes,
            processed_mb: round_to(self.processed_bytes as f64 / 1e6, 1),
            total_mb: round_to(self.total_bytes as f64 / 1e6, 1),
            percent_complete: self.percent_complete(),
        }
    }

    pub fn summary_string(&self) -> String {
        let snapshot = self.snapshot();
        format!(
            "Processed {}/{} files ({}MB/{}MB - {}%).",
            snapshot.processed_files,
            snapshot.total_files,
            snapshot.processed_mb,
            snapshot.total_mb,
            snapshot.percent_complete
        )
    }
}

fn round_to(value: f64, decimals: u32) -> f64 {
    let factor = 10_f64.powi(decimals as i32);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::ProgressTracker;

    #[test]
    fn tracks_bytes_mb_and_percentage_with_legacy_rounding() {
        let mut tracker = ProgressTracker::new(3, 1_234_567);
        tracker.record_processed(123_456);
        tracker.record_processed(500_000);

        let snapshot = tracker.snapshot();

        assert_eq!(snapshot.processed_files, 2);
        assert_eq!(snapshot.total_files, 3);
        assert_eq!(snapshot.processed_bytes, 623_456);
        assert_eq!(snapshot.total_bytes, 1_234_567);
        assert_eq!(snapshot.processed_mb, 0.6);
        assert_eq!(snapshot.total_mb, 1.2);
        assert_eq!(snapshot.percent_complete, 50.5);
    }

    #[test]
    fn zero_total_bytes_does_not_divide_by_zero() {
        let mut tracker = ProgressTracker::new(0, 0);
        tracker.record_processed(0);

        let snapshot = tracker.snapshot();

        assert_eq!(snapshot.total_mb, 0.0);
        assert_eq!(snapshot.processed_mb, 0.0);
        assert_eq!(snapshot.percent_complete, 0.0);
        assert_eq!(
            tracker.summary_string(),
            "Processed 1/0 files (0MB/0MB - 0%)."
        );
    }

    #[test]
    fn summary_string_matches_legacy_shape() {
        let mut tracker = ProgressTracker::new(4, 2_000_000);
        tracker.record_processed(1_249_900);

        assert_eq!(
            tracker.summary_string(),
            "Processed 1/4 files (1.2MB/2MB - 62.5%)."
        );
    }
}
