use chrono::{DateTime, Utc};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::exif_dates;
use crate::external_apps;

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
        media_kind: MediaKind,
    ) -> io::Result<CreationTimestamps> {
        let metadata = fs::metadata(path)?;
        let modified = metadata.modified()?;
        let metadata_ts = DateTime::<Utc>::from(modified);
        let exif = match media_kind {
            MediaKind::Image => read_exif_timestamp(path),
            MediaKind::Video => read_video_timestamp(path),
            MediaKind::Unknown => None,
        };

        Ok(CreationTimestamps {
            exif,
            metadata: Self::normalize_metadata_timestamp(metadata_ts),
        })
    }
}

use chrono::Datelike;

fn read_video_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    let output = Command::new(external_apps::ffprobe_binary())
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format_tags=creation_time:stream_tags=creation_time")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    parse_ffprobe_creation_time_output(&output.stdout)
}

fn parse_ffprobe_creation_time_output(stdout: &[u8]) -> Option<DateTime<Utc>> {
    let output = std::str::from_utf8(stdout).ok()?;
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find_map(parse_ffprobe_timestamp_line)
}

fn parse_ffprobe_timestamp_line(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(raw) {
        let utc = parsed.with_timezone(&Utc);
        return (utc.year() >= LEGACY_MIN_YEAR).then_some(utc);
    }

    let naive = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S"))
        .ok()?;

    let utc = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
    (utc.year() >= LEGACY_MIN_YEAR).then_some(utc)
}

fn read_exif_timestamp(path: &Path) -> Option<DateTime<Utc>> {
    exif_dates::read_exif_timestamp(path, Some(LEGACY_MIN_YEAR))
}

#[cfg(test)]
mod tests {
    use super::{
        CreationTimestamps, FileSystemTimestampProvider, MediaKind, TimestampProvider,
        parse_ffprobe_creation_time_output,
    };
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

    #[test]
    fn parses_video_embedded_creation_time_from_ffprobe_output() {
        let output = b"2024-07-08T09:10:11.000000Z\n";
        let parsed = parse_ffprobe_creation_time_output(output);
        let expected = Utc
            .with_ymd_and_hms(2024, 7, 8, 9, 10, 11)
            .single()
            .expect("valid datetime");
        assert_eq!(parsed, Some(expected));
    }

    #[test]
    fn video_ffprobe_output_without_creation_time_returns_none() {
        let output = b"\n\n";
        let parsed = parse_ffprobe_creation_time_output(output);
        assert_eq!(parsed, None);
    }

    #[test]
    fn malformed_video_timestamp_output_is_ignored_without_panicking() {
        let output = b"not-a-timestamp\nstill-not-one\n";
        let parsed = parse_ffprobe_creation_time_output(output);
        assert_eq!(parsed, None);
    }
}
