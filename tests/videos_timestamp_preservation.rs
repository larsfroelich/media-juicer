use filetime::{FileTime, set_file_mtime};
use media_juicer::app::execute::execute_plan;
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::image_processing::{BackendImage, ImageBackend, ImageProcessingError};
use media_juicer::media_kind::MediaKind;
use media_juicer::planning::{PlannedFile, ProcessingPlan};
use media_juicer::timestamps::{
    CreationTimestamps, MediaKind as TimestampMediaKind, TimestampProvider,
};
use media_juicer::video_processing::{FfmpegExecutor, FfmpegRunOutput, FileSizeProvider, output_path_mp4};
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitStatus;

struct NoopImageBackend;

impl ImageBackend for NoopImageBackend {
    fn open(&self, _source_path: &Path) -> Result<BackendImage, ImageProcessingError> {
        Ok(BackendImage::new(
            image::DynamicImage::new_rgba8(1, 1),
            None,
        ))
    }

    fn resize(
        &self,
        _image: &mut BackendImage,
        _max_pixels: u32,
    ) -> Result<(), ImageProcessingError> {
        Ok(())
    }

    fn save(
        &self,
        _image: &BackendImage,
        temp_output_path: &Path,
        _quality: u8,
    ) -> Result<(), ImageProcessingError> {
        fs::write(temp_output_path, b"unused")?;
        Ok(())
    }
}

struct RecordingExecutor;

impl FfmpegExecutor for RecordingExecutor {
    fn run_ffmpeg(&self, args: &[String]) -> io::Result<FfmpegRunOutput> {
        let output = args.last().expect("ffmpeg output arg should exist");
        fs::write(output, b"encoded-by-mock")?;

        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            Ok(FfmpegRunOutput {
                status: ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            Ok(FfmpegRunOutput {
                status: ExitStatus::from_raw(0),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }
}

struct FixedSizeProvider {
    source_size: u64,
    temp_size: u64,
}

impl FileSizeProvider for FixedSizeProvider {
    fn size_of(&self, path: &Path) -> io::Result<u64> {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".tmp.mp4"))
        {
            Ok(self.temp_size)
        } else {
            Ok(self.source_size)
        }
    }
}

struct NoopTimestampProvider;

impl TimestampProvider for NoopTimestampProvider {
    fn creation_timestamps(
        &self,
        _path: &Path,
        _media_kind: TimestampMediaKind,
    ) -> io::Result<CreationTimestamps> {
        Ok(CreationTimestamps {
            exif: None,
            metadata: None,
        })
    }
}

fn video_plan(source: &Path, output_path: &Path) -> ProcessingPlan {
    let size_bytes = fs::metadata(source).expect("source metadata").len();
    ProcessingPlan {
        source_root: source.parent().expect("source parent").to_path_buf(),
        out_folder_path: output_path.parent().expect("output parent").to_path_buf(),
        files: vec![PlannedFile {
            source_path: source.to_path_buf(),
            media_kind: MediaKind::Video,
            output_path: output_path.to_path_buf(),
            size_bytes,
        }],
        total_bytes_to_process: size_bytes,
    }
}

fn config_with_replace(replace: bool) -> MediaJuicerConfig {
    MediaJuicerConfig {
        mode: ProcessingMode::Videos,
        replace: replace.then(|| "true".to_string()),
        ..MediaJuicerConfig::default()
    }
}

#[test]
fn execute_plan_preserves_video_mtime_for_encode_fallback_and_replace_paths() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let expected_mtime = FileTime::from_unix_time(1_704_067_200, 0);

    let source_encode = tmp.path().join("encode.mov");
    let output_encode_base = tmp.path().join("out/encode.mov");
    fs::create_dir_all(output_encode_base.parent().expect("output dir")).expect("mkdir");
    fs::write(&source_encode, b"source-video").expect("write source");
    set_file_mtime(&source_encode, expected_mtime).expect("set source mtime");

    let encode_plan = video_plan(&source_encode, &output_encode_base);
    let mut encode_stdout = Vec::new();
    let encode_summary = execute_plan(
        &encode_plan,
        &config_with_replace(false),
        &NoopImageBackend,
        &RecordingExecutor,
        &FixedSizeProvider {
            source_size: 100,
            temp_size: 90,
        },
        &NoopTimestampProvider,
        &mut encode_stdout,
    )
    .expect("encode should succeed");
    assert!(encode_summary.failures.is_empty());

    let output_encode_mp4 = output_path_mp4(&output_encode_base);
    let encoded_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&output_encode_mp4).expect("output metadata"),
    );
    assert_eq!(encoded_mtime, expected_mtime);

    let source_fallback = tmp.path().join("fallback.mov");
    let output_fallback_base = tmp.path().join("out/fallback.mov");
    fs::write(&source_fallback, b"source-for-fallback").expect("write source");
    set_file_mtime(&source_fallback, expected_mtime).expect("set source mtime");

    let fallback_plan = video_plan(&source_fallback, &output_fallback_base);
    let mut fallback_stdout = Vec::new();
    let fallback_summary = execute_plan(
        &fallback_plan,
        &config_with_replace(false),
        &NoopImageBackend,
        &RecordingExecutor,
        &FixedSizeProvider {
            source_size: 100,
            temp_size: 101,
        },
        &NoopTimestampProvider,
        &mut fallback_stdout,
    )
    .expect("fallback should succeed");
    assert!(fallback_summary.failures.is_empty());

    let output_fallback_mp4 = output_path_mp4(&output_fallback_base);
    let fallback_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&output_fallback_mp4).expect("fallback metadata"),
    );
    assert_eq!(fallback_mtime, expected_mtime);

    let source_replace = tmp.path().join("replace.mov");
    let output_replace_base = tmp.path().join("out/replace.mov");
    fs::write(&source_replace, b"source-for-replace").expect("write source");
    set_file_mtime(&source_replace, expected_mtime).expect("set source mtime");

    let replace_plan = video_plan(&source_replace, &output_replace_base);
    let mut replace_stdout = Vec::new();
    let replace_summary = execute_plan(
        &replace_plan,
        &config_with_replace(true),
        &NoopImageBackend,
        &RecordingExecutor,
        &FixedSizeProvider {
            source_size: 100,
            temp_size: 90,
        },
        &NoopTimestampProvider,
        &mut replace_stdout,
    )
    .expect("replace should succeed");
    assert!(replace_summary.failures.is_empty());

    let replaced_source_mp4 = tmp.path().join("replace.mov.mp4");
    let replaced_mtime = FileTime::from_last_modification_time(
        &fs::metadata(&replaced_source_mp4).expect("replacement metadata"),
    );
    assert_eq!(replaced_mtime, expected_mtime);
}
