use media_juicer::app::execute::execute_plan;
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::image_processing::{BackendImage, ImageBackend, ImageProcessingError};
use media_juicer::planning::build_processing_plan;
use media_juicer::selection::Mode;
use media_juicer::timestamps::{CreationTimestamps, MediaKind as TimestampMediaKind, TimestampProvider};
use media_juicer::video_processing::{FfmpegExecutor, FfmpegRunOutput, FileSizeProvider};
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitStatus;

struct MockImageBackend;

impl ImageBackend for MockImageBackend {
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
        fs::write(temp_output_path, b"mock-image")?;
        Ok(())
    }
}

struct NoopExecutor;

impl FfmpegExecutor for NoopExecutor {
    fn run_ffmpeg(&self, _args: &[String]) -> io::Result<FfmpegRunOutput> {
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

#[test]
fn images_pipeline_builds_plan_and_executes_with_mock_backend() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source_root = tmp.path().join("source_tree");
    fs::create_dir_all(source_root.join("nested/deeper")).expect("nested dirs");

    fs::write(source_root.join("top.jpg"), b"img-top").expect("top image");
    fs::write(source_root.join("nested/photo.png"), b"img-nested").expect("nested image");
    fs::write(source_root.join("nested/deeper/clip.mp4"), b"video").expect("video");
    fs::write(source_root.join("notes.txt"), b"other").expect("other");

    let plan = build_processing_plan(&source_root, Mode::Images, None).expect("plan should build");

    assert_eq!(plan.files.len(), 2, "only image files should be selected");
    assert!(plan.out_folder_path.join("nested").is_dir());
    assert!(plan.out_folder_path.join("nested/deeper").is_dir());

    let config = MediaJuicerConfig {
        mode: ProcessingMode::Images,
        ignore_timestamps: Some("true".to_string()),
        ..MediaJuicerConfig::default()
    };

    let mut stdout = Vec::new();
    let summary = execute_plan(
        &plan,
        &config,
        &MockImageBackend,
        &NoopExecutor,
        &MetadataSizeProvider,
        &NoopTimestampProvider,
        &mut stdout,
    )
    .expect("image execution should succeed");

    assert_eq!(summary.failures.len(), 0);
    assert_eq!(summary.progress.processed_files, 2);

    assert!(plan.out_folder_path.join("top.jpg.webp").exists());
    assert!(plan.out_folder_path.join("nested/photo.png.webp").exists());

    let printed = String::from_utf8(stdout).expect("utf8 progress");
    assert!(printed.contains("Processed 2/2 files"));
}
