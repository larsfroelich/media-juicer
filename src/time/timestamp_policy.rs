use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MismatchDecision {
    HardMismatch,
    WarningMismatch,
    Acceptable,
    MissingData,
}

pub fn evaluate_mismatch(
    exif: Option<DateTime<Utc>>,
    metadata: Option<DateTime<Utc>>,
) -> MismatchDecision {
    let (exif, metadata) = match (exif, metadata) {
        (Some(exif), Some(metadata)) => (exif, metadata),
        _ => return MismatchDecision::MissingData,
    };

    let delta = exif.signed_duration_since(metadata).abs();

    if delta > Duration::hours(24) {
        MismatchDecision::HardMismatch
    } else if delta > Duration::minutes(15) {
        MismatchDecision::WarningMismatch
    } else {
        MismatchDecision::Acceptable
    }
}

#[cfg(test)]
mod tests {
    use super::{MismatchDecision, evaluate_mismatch};
    use chrono::{Duration, TimeZone, Utc};

    fn ts() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    #[test]
    fn exactly_fifteen_minutes_is_acceptable() {
        let base = ts();
        let exif = base + Duration::minutes(15);

        assert_eq!(
            evaluate_mismatch(Some(exif), Some(base)),
            MismatchDecision::Acceptable
        );
    }

    #[test]
    fn just_above_fifteen_minutes_is_warning() {
        let base = ts();
        let exif = base + Duration::minutes(15) + Duration::seconds(1);

        assert_eq!(
            evaluate_mismatch(Some(exif), Some(base)),
            MismatchDecision::WarningMismatch
        );
    }

    #[test]
    fn exactly_twenty_four_hours_is_warning() {
        let base = ts();
        let exif = base + Duration::hours(24);

        assert_eq!(
            evaluate_mismatch(Some(exif), Some(base)),
            MismatchDecision::WarningMismatch
        );
    }

    #[test]
    fn just_above_twenty_four_hours_is_hard_mismatch() {
        let base = ts();
        let exif = base + Duration::hours(24) + Duration::seconds(1);

        assert_eq!(
            evaluate_mismatch(Some(exif), Some(base)),
            MismatchDecision::HardMismatch
        );
    }

    #[test]
    fn missing_data_returns_missing_data() {
        let base = ts();

        assert_eq!(
            evaluate_mismatch(None, Some(base)),
            MismatchDecision::MissingData
        );
        assert_eq!(
            evaluate_mismatch(Some(base), None),
            MismatchDecision::MissingData
        );
        assert_eq!(evaluate_mismatch(None, None), MismatchDecision::MissingData);
    }
}
