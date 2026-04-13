use std::path::Path;
use std::time::{Duration, SystemTime};

use filetime::{FileTime, set_file_mtime};

const MAX_ALLOWED_SKEW_SECONDS: u64 = 24 * 60 * 60;

/// Legacy parity action selected when comparing EXIF and metadata dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixDateAction {
    /// Dates are close enough and no write is required.
    NoChange,
    /// EXIF is older than metadata by more than 24h; set mtime to EXIF.
    SetMtimeToExif,
    /// Metadata is older path requires writing EXIF, which is intentionally unsupported.
    UnsupportedNeedsExifWrite,
    /// Metadata date is missing; this is treated as an error.
    ErrorMissingMetadata,
}

/// Errors produced when trying to apply a [`FixDateAction`].
#[derive(Debug)]
pub enum ApplyFixDateError {
    UnsupportedAction(FixDateAction),
    Io(std::io::Error),
}

impl std::fmt::Display for ApplyFixDateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedAction(action) => {
                write!(f, "cannot apply unsupported action: {action:?}")
            }
            Self::Io(err) => write!(f, "I/O error while applying date fix: {err}"),
        }
    }
}

impl std::error::Error for ApplyFixDateError {}

impl From<std::io::Error> for ApplyFixDateError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Decide what to do with dates, mirroring the legacy Python behavior.
pub fn decide_action(
    exif_date: Option<SystemTime>,
    metadata_date: Option<SystemTime>,
) -> FixDateAction {
    let Some(metadata_date) = metadata_date else {
        return FixDateAction::ErrorMissingMetadata;
    };

    let Some(exif_date) = exif_date else {
        return FixDateAction::UnsupportedNeedsExifWrite;
    };

    let difference = exif_date.duration_since(metadata_date).unwrap_or_else(|_| {
        metadata_date
            .duration_since(exif_date)
            .unwrap_or(Duration::ZERO)
    });

    if difference.as_secs() <= MAX_ALLOWED_SKEW_SECONDS {
        return FixDateAction::NoChange;
    }

    if exif_date < metadata_date {
        FixDateAction::SetMtimeToExif
    } else {
        FixDateAction::UnsupportedNeedsExifWrite
    }
}

/// Apply a supported action to a file.
///
/// Only [`FixDateAction::NoChange`] and [`FixDateAction::SetMtimeToExif`] are executable.
pub fn apply_action(
    path: &Path,
    action: FixDateAction,
    exif_date: Option<SystemTime>,
) -> Result<(), ApplyFixDateError> {
    match action {
        FixDateAction::NoChange => Ok(()),
        FixDateAction::SetMtimeToExif => {
            let exif_date = exif_date.ok_or(ApplyFixDateError::UnsupportedAction(action))?;
            set_file_mtime(path, FileTime::from_system_time(exif_date))?;
            Ok(())
        }
        FixDateAction::UnsupportedNeedsExifWrite | FixDateAction::ErrorMissingMetadata => {
            Err(ApplyFixDateError::UnsupportedAction(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplyFixDateError, FixDateAction, apply_action, decide_action};
    use filetime::FileTime;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_file_path(test_name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("now should be after epoch")
            .as_nanos();
        path.push(format!(
            "media_juicer_{test_name}_{}_{}.tmp",
            std::process::id(),
            nanos
        ));
        path
    }

    #[test]
    fn decision_returns_error_when_metadata_missing() {
        let action = decide_action(Some(UNIX_EPOCH), None);
        assert_eq!(action, FixDateAction::ErrorMissingMetadata);
    }

    #[test]
    fn decision_returns_unsupported_when_exif_missing() {
        let action = decide_action(None, Some(UNIX_EPOCH));
        assert_eq!(action, FixDateAction::UnsupportedNeedsExifWrite);
    }

    #[test]
    fn decision_returns_no_change_within_24h_window() {
        let metadata = UNIX_EPOCH + Duration::from_secs(2_000_000);
        let exif = metadata + Duration::from_secs(60 * 60);

        let action = decide_action(Some(exif), Some(metadata));
        assert_eq!(action, FixDateAction::NoChange);
    }

    #[test]
    fn decision_prefers_older_exif_when_mismatch_exceeds_24h() {
        let exif = UNIX_EPOCH + Duration::from_secs(2_000_000);
        let metadata = exif + Duration::from_secs(25 * 60 * 60);

        let action = decide_action(Some(exif), Some(metadata));
        assert_eq!(action, FixDateAction::SetMtimeToExif);
    }

    #[test]
    fn decision_marks_metadata_older_path_as_unsupported() {
        let metadata = UNIX_EPOCH + Duration::from_secs(2_000_000);
        let exif = metadata + Duration::from_secs(25 * 60 * 60);

        let action = decide_action(Some(exif), Some(metadata));
        assert_eq!(action, FixDateAction::UnsupportedNeedsExifWrite);
    }

    #[test]
    fn apply_updates_mtime_for_supported_set_mtime_action() {
        let path = temp_file_path("set_mtime");
        fs::write(&path, b"x").expect("temp file should be created");

        let exif = UNIX_EPOCH + Duration::from_secs(1_234_567);
        apply_action(&path, FixDateAction::SetMtimeToExif, Some(exif))
            .expect("apply should succeed");

        let actual = FileTime::from_last_modification_time(
            &fs::metadata(&path).expect("metadata should be readable"),
        );
        let expected = FileTime::from_system_time(exif);

        assert_eq!(actual.seconds(), expected.seconds());

        fs::remove_file(path).expect("temp file should be removable");
    }

    #[test]
    fn apply_rejects_unsupported_actions() {
        let path = temp_file_path("unsupported");
        fs::write(&path, b"x").expect("temp file should be created");

        let result = apply_action(&path, FixDateAction::UnsupportedNeedsExifWrite, None);
        assert!(matches!(
            result,
            Err(ApplyFixDateError::UnsupportedAction(
                FixDateAction::UnsupportedNeedsExifWrite
            ))
        ));

        fs::remove_file(path).expect("temp file should be removable");
    }
}
