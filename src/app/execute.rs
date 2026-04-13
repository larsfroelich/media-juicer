use std::fmt::{Display, Formatter};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::config::{MediaJuicerConfig, ProcessingMode};
use crate::fix_dates::{apply_action, decide_action};
use crate::image_processing::{ImageBackend, ImageJob, process_image_job};
use crate::media_kind::MediaKind;
use crate::planning::{PlannedFile, ProcessingPlan};
use crate::progress::{ProgressSnapshot, ProgressTracker};
use crate::timestamps::{MediaKind as TimestampMediaKind, TimestampProvider};
use crate::video_processing::{
    FfmpegExecutor, FileSizeProvider, VideoJob, apply_replace_input, process_video,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFailure {
    pub path: PathBuf,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionSummary {
    pub progress: ProgressSnapshot,
    pub failures: Vec<FileFailure>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    FileFailures(ExecutionSummary),
    ReportIo(String),
}

impl Display for ExecutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileFailures(summary) => {
                write!(
                    f,
                    "{} file(s) failed during execution",
                    summary.failures.len()
                )
            }
            Self::ReportIo(error) => write!(f, "failed to write execution report: {error}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

pub fn execute_plan<B: ImageBackend, T: TimestampProvider>(
    plan: &ProcessingPlan,
    config: &MediaJuicerConfig,
    image_backend: &B,
    ffmpeg_executor: &dyn FfmpegExecutor,
    file_size_provider: &dyn FileSizeProvider,
    timestamp_provider: &T,
    out: &mut dyn Write,
) -> Result<ExecutionSummary, ExecutionError> {
    let selected_files: Vec<&PlannedFile> = plan
        .files
        .iter()
        .filter(|file| is_handled_by_mode(config.mode, file.media_kind))
        .collect();

    let total_bytes = selected_files.iter().map(|file| file.size_bytes).sum();
    let mut progress = ProgressTracker::new(selected_files.len(), total_bytes);

    let mut failures = Vec::new();

    for file in &plan.files {
        if !is_handled_by_mode(config.mode, file.media_kind) {
            continue;
        }

        if let Err(error) = process_file(
            file,
            config,
            image_backend,
            ffmpeg_executor,
            file_size_provider,
            timestamp_provider,
        ) {
            failures.push(FileFailure {
                path: file.source_path.clone(),
                error,
            });
        }

        progress.record_processed(file.size_bytes);
        writeln!(out, "{}", progress.summary_string())
            .map_err(|err| ExecutionError::ReportIo(err.to_string()))?;
    }

    if !failures.is_empty() {
        writeln!(out, "Failed to process {} file(s):", failures.len())
            .map_err(|err| ExecutionError::ReportIo(err.to_string()))?;

        for failure in &failures {
            writeln!(
                out,
                "- \"{}\": {}",
                failure.path.to_string_lossy(),
                failure.error
            )
            .map_err(|err| ExecutionError::ReportIo(err.to_string()))?;
        }
    }

    let summary = ExecutionSummary {
        progress: progress.snapshot(),
        failures,
    };

    if summary.failures.is_empty() {
        Ok(summary)
    } else {
        Err(ExecutionError::FileFailures(summary))
    }
}

fn process_file<B: ImageBackend, T: TimestampProvider>(
    file: &PlannedFile,
    config: &MediaJuicerConfig,
    image_backend: &B,
    ffmpeg_executor: &dyn FfmpegExecutor,
    file_size_provider: &dyn FileSizeProvider,
    timestamp_provider: &T,
) -> Result<(), String> {
    match config.mode {
        ProcessingMode::All | ProcessingMode::Images if file.media_kind == MediaKind::Image => {
            process_image(file, config, image_backend)
        }
        ProcessingMode::All | ProcessingMode::Videos if file.media_kind == MediaKind::Video => {
            process_video_file(file, config, ffmpeg_executor, file_size_provider)
        }
        ProcessingMode::FixDates => process_fix_dates(file, config, timestamp_provider),
        _ => Ok(()),
    }
}

fn process_image<B: ImageBackend>(
    file: &PlannedFile,
    config: &MediaJuicerConfig,
    image_backend: &B,
) -> Result<(), String> {
    let quality = u8::try_from(config.webpq)
        .map_err(|_| format!("invalid webp quality: {}", config.webpq))?;
    let max_pixels = u32::try_from(config.image_max_pixels)
        .map_err(|_| format!("invalid image max pixels: {}", config.image_max_pixels))?;

    let job = ImageJob {
        source_path: file.source_path.clone(),
        output_path: file.output_path.clone(),
        quality,
        max_pixels,
        ignore_timestamps: config.ignore_timestamps.is_some(),
    };

    process_image_job(&job, image_backend)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn process_video_file(
    file: &PlannedFile,
    config: &MediaJuicerConfig,
    ffmpeg_executor: &dyn FfmpegExecutor,
    file_size_provider: &dyn FileSizeProvider,
) -> Result<(), String> {
    let crf = u8::try_from(config.crf).map_err(|_| format!("invalid crf value: {}", config.crf))?;
    let video_max_pixels = u32::try_from(config.video_max_pixels)
        .map_err(|_| format!("invalid video max pixels: {}", config.video_max_pixels))?;

    let replace = config.replace.is_some();

    let job = VideoJob {
        src_file: file.source_path.clone(),
        new_file_path: file.output_path.clone(),
        crf,
        ffmpeg_speed: config.ffmpeg_speed.to_string(),
        video_max_pixels,
        replace,
    };

    process_video(&job, ffmpeg_executor, file_size_provider).map_err(|error| error.to_string())?;

    let output_path = crate::video_processing::output_path_mp4(Path::new(&file.output_path));
    apply_replace_input(&file.source_path, &output_path, replace)
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn process_fix_dates<T: TimestampProvider>(
    file: &PlannedFile,
    config: &MediaJuicerConfig,
    timestamp_provider: &T,
) -> Result<(), String> {
    let timestamps = match timestamp_provider.creation_timestamps(
        &file.source_path,
        timestamp_kind(file.media_kind),
    ) {
        Ok(timestamps) => timestamps,
        Err(_error) if config.ignore_timestamps.is_some() => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };

    let exif = timestamps.exif.map(SystemTime::from);
    let metadata = timestamps.metadata.map(SystemTime::from);

    let action = decide_action(exif, metadata);
    match apply_action(&file.source_path, action, exif) {
        Ok(()) => Ok(()),
        Err(_error) if config.ignore_timestamps.is_some() => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn timestamp_kind(kind: MediaKind) -> TimestampMediaKind {
    match kind {
        MediaKind::Image => TimestampMediaKind::Image,
        MediaKind::Video => TimestampMediaKind::Video,
        MediaKind::Other => TimestampMediaKind::Unknown,
    }
}

fn is_handled_by_mode(mode: ProcessingMode, media_kind: MediaKind) -> bool {
    matches!(
        (mode, media_kind),
        (ProcessingMode::All, MediaKind::Image | MediaKind::Video)
            | (ProcessingMode::Images, MediaKind::Image)
            | (ProcessingMode::Videos, MediaKind::Video)
            | (
                ProcessingMode::FixDates,
                MediaKind::Image | MediaKind::Video
            )
    )
}

#[cfg(test)]
mod tests {
    use super::{ExecutionError, execute_plan};
    use crate::config::{FfmpegPreset, MediaJuicerConfig, ProcessingMode};
    use crate::image_processing::{BackendImage, ImageBackend, ImageProcessingError};
    use crate::media_kind::MediaKind;
    use crate::planning::{PlannedFile, ProcessingPlan};
    use crate::timestamps::{
        CreationTimestamps, MediaKind as TimestampMediaKind, TimestampProvider,
    };
    use crate::video_processing::{FfmpegExecutor, FfmpegRunOutput, FileSizeProvider};
    use chrono::Utc;
    use std::io;
    use std::path::Path;
    use std::process::ExitStatus;

    struct OkImageBackend;

    impl ImageBackend for OkImageBackend {
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
            std::fs::write(temp_output_path, b"ok")?;
            Ok(())
        }
    }

    struct NoopExecutor;

    #[cfg(unix)]
    fn success_status() -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn success_status() -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }

    impl FfmpegExecutor for NoopExecutor {
        fn run_ffmpeg(&self, args: &[String]) -> io::Result<FfmpegRunOutput> {
            std::fs::write(args.last().expect("output path"), b"video")?;
            Ok(FfmpegRunOutput {
                status: success_status(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    struct SizeProvider;

    impl FileSizeProvider for SizeProvider {
        fn size_of(&self, _path: &Path) -> io::Result<u64> {
            Ok(1)
        }
    }

    struct FailingTimestampProvider;

    impl TimestampProvider for FailingTimestampProvider {
        fn creation_timestamps(
            &self,
            _path: &Path,
            _media_kind: TimestampMediaKind,
        ) -> io::Result<CreationTimestamps> {
            Err(io::Error::other("missing metadata"))
        }
    }

    struct StaticTimestampProvider;

    impl TimestampProvider for StaticTimestampProvider {
        fn creation_timestamps(
            &self,
            _path: &Path,
            _media_kind: TimestampMediaKind,
        ) -> io::Result<CreationTimestamps> {
            Ok(CreationTimestamps {
                exif: Some(Utc::now()),
                metadata: Some(Utc::now()),
            })
        }
    }

    fn config(mode: ProcessingMode) -> MediaJuicerConfig {
        MediaJuicerConfig {
            folder_path: "/tmp".to_string(),
            verbose: false,
            mode,
            replace: None,
            only: None,
            ignore_timestamps: None,
            crf: 28,
            ffmpeg_speed: FfmpegPreset::Faster,
            video_max_pixels: 0,
            webpq: 45,
            image_max_pixels: 1600,
        }
    }

    #[test]
    fn prints_progress_for_each_handled_file() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("img.jpg");
        std::fs::write(&source, b"img").unwrap();

        let plan = ProcessingPlan {
            source_root: tmp.path().to_path_buf(),
            out_folder_path: tmp.path().join("out"),
            files: vec![PlannedFile {
                source_path: source,
                media_kind: MediaKind::Image,
                output_path: tmp.path().join("out/img.jpg"),
                size_bytes: 3,
            }],
            total_bytes_to_process: 3,
        };

        let mut out = Vec::new();
        let result = execute_plan(
            &plan,
            &config(ProcessingMode::Images),
            &OkImageBackend,
            &NoopExecutor,
            &SizeProvider,
            &StaticTimestampProvider,
            &mut out,
        )
        .unwrap();

        assert_eq!(result.progress.processed_files, 1);
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("Processed 1/1 files (0MB/0MB - 100%)."));
    }

    #[test]
    fn continues_after_recoverable_errors_and_returns_aggregated_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let source_a = tmp.path().join("a.jpg");
        let source_b = tmp.path().join("b.jpg");
        std::fs::write(&source_a, b"a").unwrap();
        std::fs::write(&source_b, b"b").unwrap();

        let plan = ProcessingPlan {
            source_root: tmp.path().to_path_buf(),
            out_folder_path: tmp.path().join("out"),
            files: vec![
                PlannedFile {
                    source_path: source_a,
                    media_kind: MediaKind::Image,
                    output_path: tmp.path().join("out/a.jpg"),
                    size_bytes: 1,
                },
                PlannedFile {
                    source_path: source_b,
                    media_kind: MediaKind::Image,
                    output_path: tmp.path().join("out/b.jpg"),
                    size_bytes: 1,
                },
            ],
            total_bytes_to_process: 2,
        };

        let mut out = Vec::new();
        let result = execute_plan(
            &plan,
            &config(ProcessingMode::FixDates),
            &OkImageBackend,
            &NoopExecutor,
            &SizeProvider,
            &FailingTimestampProvider,
            &mut out,
        );

        let ExecutionError::FileFailures(summary) = result.expect_err("should fail") else {
            panic!("expected file failure report");
        };

        assert_eq!(summary.progress.processed_files, 2);
        assert_eq!(summary.failures.len(), 2);
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("Processed 2/2 files"));
        assert!(printed.contains("Failed to process 2 file(s):"));
    }

    #[test]
    fn skips_unhandled_files_even_when_present_in_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("clip.mp4");
        std::fs::write(&source, b"vid").unwrap();

        let plan = ProcessingPlan {
            source_root: tmp.path().to_path_buf(),
            out_folder_path: tmp.path().join("out"),
            files: vec![PlannedFile {
                source_path: source,
                media_kind: MediaKind::Video,
                output_path: tmp.path().join("out/clip.mp4"),
                size_bytes: 3,
            }],
            total_bytes_to_process: 3,
        };

        let mut out = Vec::new();
        let result = execute_plan(
            &plan,
            &config(ProcessingMode::Images),
            &OkImageBackend,
            &NoopExecutor,
            &SizeProvider,
            &StaticTimestampProvider,
            &mut out,
        )
        .unwrap();

        assert_eq!(result.progress.processed_files, 0);
        assert!(String::from_utf8(out).unwrap().is_empty());
    }
}
