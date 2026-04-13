use chrono::{DateTime, Utc};
use std::fs;
use std::io;
use std::path::Path;

const LEGACY_MIN_YEAR: i32 = 1980;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationTimestamps {
    pub exif: Option<DateTime<Utc>>,
    pub metadata: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Unknown,
}

pub trait TimestampProvider {
    fn creation_timestamps(
        &self,
        path: &Path,
        media_kind: MediaKind,
    ) -> io::Result<CreationTimestamps>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemTimestampProvider;

impl FileSystemTimestampProvider {
    fn normalize_metadata_timestamp(timestamp: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if timestamp.year() < LEGACY_MIN_YEAR {
            None
        } else {
            Some(timestamp)
        }
    }
}

impl TimestampProvider for FileSystemTimestampProvider {
    fn creation_timestamps(
        &self,
        path: &Path,
        _media_kind: MediaKind,
    ) -> io::Result<CreationTimestamps> {
        let metadata = fs::metadata(path)?;
        let modified = metadata.modified()?;
        let metadata_ts = DateTime::<Utc>::from(modified);

        Ok(CreationTimestamps {
            exif: None,
            metadata: Self::normalize_metadata_timestamp(metadata_ts),
        })
    }
}

use chrono::Datelike;

#[cfg(test)]
mod tests {
    use super::{CreationTimestamps, FileSystemTimestampProvider, MediaKind, TimestampProvider};
    use chrono::{Datelike, TimeZone, Utc};
    use filetime::{FileTime, set_file_mtime};
    use std::fs::File;
    use std::io;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("media_juicer_{name}_{nanos}.tmp"))
    }

    #[test]
    fn converts_mtime_to_metadata_timestamp() -> io::Result<()> {
        let path = temp_file_path("normal_mtime");
        let _file = File::create(&path)?;

        let expected = Utc
            .with_ymd_and_hms(2020, 1, 2, 3, 4, 5)
            .single()
            .expect("valid datetime");
        set_file_mtime(
            &path,
            FileTime::from_unix_time(expected.timestamp(), expected.timestamp_subsec_nanos()),
        )?;

        let provider = FileSystemTimestampProvider;
        let result = provider.creation_timestamps(&path, MediaKind::Image)?;

        assert_eq!(
            result,
            CreationTimestamps {
                exif: None,
                metadata: Some(expected)
            }
        );

        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn rejects_implausibly_old_metadata_timestamps() -> io::Result<()> {
        let path = temp_file_path("old_mtime");
        let _file = File::create(&path)?;

        let too_old = Utc
            .with_ymd_and_hms(1979, 12, 31, 23, 59, 59)
            .single()
            .expect("valid datetime");
        set_file_mtime(
            &path,
            FileTime::from_unix_time(too_old.timestamp(), too_old.timestamp_subsec_nanos()),
        )?;

        let provider = FileSystemTimestampProvider;
        let result = provider.creation_timestamps(&path, MediaKind::Video)?;

        assert!(result.exif.is_none());
        assert!(result.metadata.is_none());

        std::fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn propagates_errors_for_missing_files() {
        let provider = FileSystemTimestampProvider;
        let missing_path = temp_file_path("missing");

        let err = provider
            .creation_timestamps(&missing_path, MediaKind::Unknown)
            .expect_err("missing file should return an io error");

        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn boundary_year_is_accepted() -> io::Result<()> {
        let path = temp_file_path("boundary");
        let _file = File::create(&path)?;

        let boundary = Utc
            .with_ymd_and_hms(1980, 1, 1, 0, 0, 0)
            .single()
            .expect("valid datetime");
        set_file_mtime(
            &path,
            FileTime::from_unix_time(boundary.timestamp(), boundary.timestamp_subsec_nanos()),
        )?;

        let provider = FileSystemTimestampProvider;
        let result = provider.creation_timestamps(&path, MediaKind::Image)?;

        assert_eq!(
            result.metadata.expect("metadata should be present").year(),
            1980
        );

        std::fs::remove_file(path)?;
        Ok(())
    }
}
