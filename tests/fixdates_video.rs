mod common;

use chrono::TimeZone;
use common::{ConstantSizeProvider, MockImageBackend, NoopFfmpegExecutor};
use filetime::{FileTime, set_file_mtime};
use media_juicer::app::execute::execute_plan;
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::media_kind::MediaKind;
use media_juicer::planning::{PlannedFile, ProcessingPlan};
use media_juicer::timestamps::FileSystemTimestampProvider;

#[test]
fn fixdates_video_succeeds_when_ffprobe_provides_embedded_creation_time() {
    let tmp = tempfile::tempdir().expect("temp dir should exist");
    let source = tmp.path().join("clip.mp4");
    std::fs::write(&source, b"video").expect("video file should be writable");

    let ffprobe_script = tmp.path().join("ffprobe-mock.sh");
    std::fs::write(
        &ffprobe_script,
        "#!/usr/bin/env sh\necho '2024-01-02T03:04:05.000000Z'\n",
    )
    .expect("ffprobe script should be writable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&ffprobe_script)
            .expect("script metadata should be readable")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&ffprobe_script, permissions)
            .expect("script should become executable");
    }

    let target_mtime = chrono::Utc
        .with_ymd_and_hms(2024, 1, 2, 3, 4, 5)
        .single()
        .expect("valid timestamp");
    set_file_mtime(
        &source,
        FileTime::from_unix_time(
            target_mtime.timestamp(),
            target_mtime.timestamp_subsec_nanos(),
        ),
    )
    .expect("mtime should be writable");

    unsafe {
        std::env::set_var("MEDIA_JUICER_FFPROBE", &ffprobe_script);
    }

    let plan = ProcessingPlan {
        source_root: tmp.path().to_path_buf(),
        out_folder_path: tmp.path().join("out"),
        files: vec![PlannedFile {
            source_path: source,
            media_kind: MediaKind::Video,
            output_path: tmp.path().join("out/clip.mp4"),
            size_bytes: 5,
        }],
        total_bytes_to_process: 5,
    };

    let config = MediaJuicerConfig {
        mode: ProcessingMode::FixDates,
        ..MediaJuicerConfig::default()
    };
    let mut out = Vec::new();

    let result = execute_plan(
        &plan,
        &config,
        &MockImageBackend,
        &NoopFfmpegExecutor,
        &ConstantSizeProvider { size: 1 },
        &FileSystemTimestampProvider,
        &mut out,
    );

    unsafe {
        std::env::remove_var("MEDIA_JUICER_FFPROBE");
    }
    assert!(
        result.is_ok(),
        "fixdates should succeed when video timestamp exists"
    );
}
