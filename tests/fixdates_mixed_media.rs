mod common;

use chrono::Utc;
use common::{MetadataSizeProvider, MockImageBackend, NoopFfmpegExecutor};
use media_juicer::app::execute::{ExecutionError, execute_plan};
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::planning::build_processing_plan;
use media_juicer::selection::Mode;
use media_juicer::timestamps::{
    CreationTimestamps, MediaKind as TimestampMediaKind, TimestampProvider,
};
use std::fs;
use std::io;
use std::path::Path;

struct SelectiveTimestampProvider;

impl TimestampProvider for SelectiveTimestampProvider {
    fn creation_timestamps(
        &self,
        path: &Path,
        _media_kind: TimestampMediaKind,
    ) -> io::Result<CreationTimestamps> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if name.contains("fail") {
            return Err(io::Error::other("mock timestamp lookup failed"));
        }

        Ok(CreationTimestamps {
            exif: Some(Utc::now()),
            metadata: Some(Utc::now()),
        })
    }
}

#[test]
fn fixdates_aggregates_failures_for_mixed_media_and_reports_full_progress() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source_root = tmp.path().join("mixed");
    fs::create_dir_all(source_root.join("nested")).expect("nested dir");

    fs::write(source_root.join("ok_image.jpg"), b"image-ok").expect("ok image");
    fs::write(source_root.join("fail_video.mp4"), b"video-fail").expect("fail video");
    fs::write(source_root.join("nested/fail_image.png"), b"image-fail").expect("fail image");

    let plan =
        build_processing_plan(&source_root, Mode::Fixdates, None).expect("plan should build");
    assert_eq!(plan.files.len(), 3);

    let config = MediaJuicerConfig {
        mode: ProcessingMode::FixDates,
        ..MediaJuicerConfig::default()
    };

    let mut stdout = Vec::new();
    let result = execute_plan(
        &plan,
        &config,
        &MockImageBackend,
        &NoopFfmpegExecutor,
        &MetadataSizeProvider,
        &SelectiveTimestampProvider,
        &mut stdout,
    );

    let ExecutionError::FileFailures(summary) = result.expect_err("should aggregate failures")
    else {
        panic!("expected aggregated file failures");
    };

    assert_eq!(summary.progress.processed_files, 3);
    assert_eq!(summary.failures.len(), 2);

    let failure_paths: Vec<_> = summary
        .failures
        .iter()
        .map(|failure| {
            failure
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert!(failure_paths.contains(&"fail_video.mp4".to_string()));
    assert!(failure_paths.contains(&"fail_image.png".to_string()));

    let printed = String::from_utf8(stdout).expect("utf8");
    assert!(printed.contains("Processed 3/3 files"));
    assert!(printed.contains("Failed to process 2 file(s):"));
}

#[test]
fn fixdates_ignore_timestamps_suppresses_timestamp_lookup_failures() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source_root = tmp.path().join("mixed_ignore");
    fs::create_dir_all(&source_root).expect("root dir");

    fs::write(source_root.join("fail_video.mp4"), b"video-fail").expect("fail video");
    fs::write(source_root.join("fail_image.png"), b"image-fail").expect("fail image");

    let plan =
        build_processing_plan(&source_root, Mode::Fixdates, None).expect("plan should build");
    assert_eq!(plan.files.len(), 2);

    let config = MediaJuicerConfig {
        mode: ProcessingMode::FixDates,
        ignore_timestamps: Some("true".to_string()),
        ..MediaJuicerConfig::default()
    };

    let mut stdout = Vec::new();
    let summary = execute_plan(
        &plan,
        &config,
        &MockImageBackend,
        &NoopFfmpegExecutor,
        &MetadataSizeProvider,
        &SelectiveTimestampProvider,
        &mut stdout,
    )
    .expect("ignore-timestamps should suppress fixdate failures");

    assert_eq!(summary.progress.processed_files, 2);
    assert!(summary.failures.is_empty());
}
