use media_juicer::app::execute::execute_plan;
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::image_processing::{BackendImage, ImageBackend, ImageProcessingError};
use media_juicer::media_kind::MediaKind;
use media_juicer::planning::{PlannedFile, ProcessingPlan};
use media_juicer::timestamps::{CreationTimestamps, MediaKind as TimestampMediaKind, TimestampProvider};
use media_juicer::video_processing::{output_path_mp4, temp_output_path, FfmpegExecutor, FfmpegRunOutput, FileSizeProvider};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

struct NoopImageBackend;

impl ImageBackend for NoopImageBackend {
    fn open(&self, _source_path: &Path) -> Result<BackendImage, ImageProcessingError> {
        Ok(BackendImage::new(image::DynamicImage::new_rgba8(1, 1), None))
    }

    fn resize(&self, _image: &mut BackendImage, _max_pixels: u32) -> Result<(), ImageProcessingError> {
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

struct MetadataSizeProvider;

impl FileSizeProvider for MetadataSizeProvider {
    fn size_of(&self, path: &Path) -> io::Result<u64> {
        Ok(fs::metadata(path)?.len())
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
        out_folder_path: output_path
            .parent()
            .expect("output parent")
            .to_path_buf(),
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
fn replace_off_keeps_existing_output_and_source() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source = tmp.path().join("clip.mov");
    let out_base = tmp.path().join("out/clip.mov");
    fs::create_dir_all(out_base.parent().expect("out parent")).expect("mkdir out");

    fs::write(&source, b"original-source").expect("write source");
    let output_mp4 = output_path_mp4(&out_base);
    fs::write(&output_mp4, b"existing-output").expect("write existing output");

    let plan = video_plan(&source, &out_base);
    let mut stdout = Vec::new();

    let summary = execute_plan(
        &plan,
        &config_with_replace(false),
        &NoopImageBackend,
        &RecordingExecutor,
        &MetadataSizeProvider,
        &NoopTimestampProvider,
        &mut stdout,
    )
    .expect("execution should succeed");

    assert_eq!(summary.failures.len(), 0);
    assert!(source.exists());
    assert_eq!(fs::read(&output_mp4).expect("read output"), b"existing-output");
    assert!(!temp_output_path(&output_mp4).exists());
}

#[test]
fn replace_on_uses_existing_output_and_replaces_non_mp4_source_with_mp4_copy() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source = tmp.path().join("clip.mov");
    let out_base = tmp.path().join("out/clip.mov");
    fs::create_dir_all(out_base.parent().expect("out parent")).expect("mkdir out");

    fs::write(&source, b"original-source").expect("write source");
    let output_mp4 = output_path_mp4(&out_base);
    fs::write(&output_mp4, b"already-encoded").expect("write existing output");

    let plan = video_plan(&source, &out_base);
    let mut stdout = Vec::new();

    let summary = execute_plan(
        &plan,
        &config_with_replace(true),
        &NoopImageBackend,
        &RecordingExecutor,
        &MetadataSizeProvider,
        &NoopTimestampProvider,
        &mut stdout,
    )
    .expect("execution should succeed");

    assert_eq!(summary.failures.len(), 0);
    assert!(!source.exists(), "source should be removed when replace is on");

    let replaced_source_mp4 = PathBuf::from(format!("{}.mp4", source.to_string_lossy()));
    assert!(replaced_source_mp4.exists());
    assert_eq!(fs::read(replaced_source_mp4).expect("read replacement"), b"already-encoded");
    assert_eq!(fs::read(&output_mp4).expect("read output"), b"already-encoded");
}

#[test]
fn non_mp4_output_path_is_encoded_to_mp4_target() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source = tmp.path().join("fresh.mov");
    let out_base = tmp.path().join("out/fresh.mov");
    fs::create_dir_all(out_base.parent().expect("out parent")).expect("mkdir out");

    fs::write(&source, b"source-video-content").expect("write source");
    let output_mp4 = output_path_mp4(&out_base);

    let plan = video_plan(&source, &out_base);
    let mut stdout = Vec::new();

    let summary = execute_plan(
        &plan,
        &config_with_replace(false),
        &NoopImageBackend,
        &RecordingExecutor,
        &MetadataSizeProvider,
        &NoopTimestampProvider,
        &mut stdout,
    )
    .expect("execution should succeed");

    assert_eq!(summary.failures.len(), 0);
    assert!(output_mp4.exists(), "output should be normalized to .mp4");
    assert_eq!(fs::read(output_mp4).expect("read output"), b"encoded-by-mock");
}
