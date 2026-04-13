use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

/// Configuration for a single video compression job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoJob {
    pub src_file: PathBuf,
    pub new_file_path: PathBuf,
    pub crf: u8,
    pub ffmpeg_speed: String,
    pub video_max_pixels: u32,
    pub replace: bool,
}

/// A serializable command result for ffmpeg executions.
#[derive(Debug, Clone)]
pub struct FfmpegRunOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub trait FfmpegExecutor {
    fn run_ffmpeg(&self, args: &[String]) -> io::Result<FfmpegRunOutput>;
}

/// System executor that shells out to ffmpeg.
pub struct SystemFfmpegExecutor;

impl FfmpegExecutor for SystemFfmpegExecutor {
    fn run_ffmpeg(&self, args: &[String]) -> io::Result<FfmpegRunOutput> {
        let output = Command::new("ffmpeg").args(args).output()?;
        Ok(FfmpegRunOutput {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

pub trait FileSizeProvider {
    fn size_of(&self, path: &Path) -> io::Result<u64>;
}

pub struct StdFileSizeProvider;

impl FileSizeProvider for StdFileSizeProvider {
    fn size_of(&self, path: &Path) -> io::Result<u64> {
        Ok(fs::metadata(path)?.len())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessDecision {
    SkipExisting,
    UseExistingForReplace,
    Encode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOutcome {
    SkippedExisting,
    UsedExisting,
    Encoded,
    EncodedWithFallbackCopy,
}

pub fn output_path_mp4(path: &Path) -> PathBuf {
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(OsStr::new("mp4")))
    {
        path.to_path_buf()
    } else {
        PathBuf::from(format!("{}.mp4", path.to_string_lossy()))
    }
}

pub fn temp_output_path(output_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp.mp4", output_path.to_string_lossy()))
}

pub fn processing_decision(output_exists: bool, replace: bool) -> ProcessDecision {
    if output_exists {
        if replace {
            ProcessDecision::UseExistingForReplace
        } else {
            ProcessDecision::SkipExisting
        }
    } else {
        ProcessDecision::Encode
    }
}

/// Builds ffmpeg arguments matching the legacy flag order.
pub fn build_ffmpeg_args(job: &VideoJob) -> Vec<String> {
    let temp_path = temp_output_path(&output_path_mp4(&job.new_file_path));
    let mut args = vec![
        "-i".to_string(),
        job.src_file.to_string_lossy().into_owned(),
        "-map_metadata".to_string(),
        "0".to_string(),
        "-c:v".to_string(),
        "libx265".to_string(),
        "-x265-params".to_string(),
        format!("crf={}", job.crf),
        "-preset".to_string(),
        job.ffmpeg_speed.clone(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-tune".to_string(),
        "fastdecode".to_string(),
    ];

    if job.video_max_pixels > 0 {
        args.push("-filter:v".to_string());
        args.push(format!(
            "scale='min({},iw)':min'({},ih)':force_original_aspect_ratio=decrease",
            job.video_max_pixels, job.video_max_pixels
        ));
    }

    args.push(temp_path.to_string_lossy().into_owned());
    args
}

pub fn process_video(
    job: &VideoJob,
    executor: &dyn FfmpegExecutor,
    size_provider: &dyn FileSizeProvider,
) -> io::Result<ProcessOutcome> {
    let output_path = output_path_mp4(&job.new_file_path);
    let temp_path = temp_output_path(&output_path);
    match processing_decision(output_path.exists(), job.replace) {
        ProcessDecision::SkipExisting => Ok(ProcessOutcome::SkippedExisting),
        ProcessDecision::UseExistingForReplace => Ok(ProcessOutcome::UsedExisting),
        ProcessDecision::Encode => {
            if temp_path.exists() {
                fs::remove_file(&temp_path)?;
            }

            let args = build_ffmpeg_args(job);
            let ffmpeg_result = executor.run_ffmpeg(&args)?;
            if !ffmpeg_result.status.success() {
                return Err(io::Error::other(format!(
                    "ffmpeg failed (status: {:?})\nstdout:{}\nstderr:{}",
                    ffmpeg_result.status,
                    String::from_utf8_lossy(&ffmpeg_result.stdout),
                    String::from_utf8_lossy(&ffmpeg_result.stderr)
                )));
            }

            let input_size = size_provider.size_of(&job.src_file)?;
            let output_size = size_provider.size_of(&temp_path)?;

            if output_size > input_size {
                fs::remove_file(&temp_path)?;
                fs::copy(&job.src_file, &temp_path)?;
                fs::rename(&temp_path, &output_path)?;
                Ok(ProcessOutcome::EncodedWithFallbackCopy)
            } else {
                fs::rename(&temp_path, &output_path)?;
                Ok(ProcessOutcome::Encoded)
            }
        }
    }
}

/// Applies legacy `--replace` behavior as a separate post-processing step.
pub fn apply_replace_input(src_file: &Path, output_path: &Path, replace: bool) -> io::Result<bool> {
    if !(replace && output_path.exists()) {
        return Ok(false);
    }

    fs::remove_file(src_file)?;

    let replacement_target = if src_file
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(OsStr::new("mp4")))
    {
        src_file.to_path_buf()
    } else {
        PathBuf::from(format!("{}.mp4", src_file.to_string_lossy()))
    };

    fs::copy(output_path, replacement_target)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

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

    struct MockExecutor;

    impl FfmpegExecutor for MockExecutor {
        fn run_ffmpeg(&self, args: &[String]) -> io::Result<FfmpegRunOutput> {
            let out_path = args.last().expect("expected output path as final ffmpeg arg");
            fs::write(out_path, b"mock-encoded")?;
            Ok(FfmpegRunOutput {
                status: success_status(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            })
        }
    }

    struct MockSizeProvider {
        sizes: Mutex<HashMap<PathBuf, u64>>,
    }

    impl MockSizeProvider {
        fn new(sizes: HashMap<PathBuf, u64>) -> Self {
            Self {
                sizes: Mutex::new(sizes),
            }
        }
    }

    impl FileSizeProvider for MockSizeProvider {
        fn size_of(&self, path: &Path) -> io::Result<u64> {
            let sizes = self.sizes.lock().expect("poisoned");
            sizes
                .get(path)
                .copied()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "mocked size missing"))
        }
    }

    #[test]
    fn build_ffmpeg_args_without_resize_matches_legacy_order() {
        let job = VideoJob {
            src_file: PathBuf::from("input.mov"),
            new_file_path: PathBuf::from("out"),
            crf: 28,
            ffmpeg_speed: "faster".to_string(),
            video_max_pixels: 0,
            replace: false,
        };

        let args = build_ffmpeg_args(&job);
        assert_eq!(
            args,
            vec![
                "-i",
                "input.mov",
                "-map_metadata",
                "0",
                "-c:v",
                "libx265",
                "-x265-params",
                "crf=28",
                "-preset",
                "faster",
                "-c:a",
                "aac",
                "-tune",
                "fastdecode",
                "out.mp4.tmp.mp4",
            ]
        );
    }

    #[test]
    fn build_ffmpeg_args_with_resize_includes_filter() {
        let job = VideoJob {
            src_file: PathBuf::from("input.mov"),
            new_file_path: PathBuf::from("out.mp4"),
            crf: 30,
            ffmpeg_speed: "slow".to_string(),
            video_max_pixels: 1600,
            replace: false,
        };

        let args = build_ffmpeg_args(&job);
        assert_eq!(args[14], "-filter:v");
        assert_eq!(
            args[15],
            "scale='min(1600,iw)':min'(1600,ih)':force_original_aspect_ratio=decrease"
        );
        assert_eq!(args.last().expect("missing output"), "out.mp4.tmp.mp4");
    }

    #[test]
    fn processing_decision_obeys_skip_and_replace_semantics() {
        assert_eq!(
            processing_decision(true, false),
            ProcessDecision::SkipExisting
        );
        assert_eq!(
            processing_decision(true, true),
            ProcessDecision::UseExistingForReplace
        );
        assert_eq!(processing_decision(false, false), ProcessDecision::Encode);
    }

    #[test]
    fn larger_output_falls_back_to_copy() {
        let temp_dir = std::env::temp_dir().join(format!(
            "media_juicer_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time monotonic")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        let src = temp_dir.join("clip.mov");
        fs::write(&src, b"source-content").expect("write src");

        let job = VideoJob {
            src_file: src.clone(),
            new_file_path: temp_dir.join("result"),
            crf: 28,
            ffmpeg_speed: "faster".to_string(),
            video_max_pixels: 0,
            replace: false,
        };

        let output = output_path_mp4(&job.new_file_path);
        let tmp = temp_output_path(&output);
        let sizes = HashMap::from([(src.clone(), 10_u64), (tmp.clone(), 20_u64)]);
        let provider = MockSizeProvider::new(sizes);
        let exec = MockExecutor;

        let outcome = process_video(&job, &exec, &provider).expect("process result");
        assert_eq!(outcome, ProcessOutcome::EncodedWithFallbackCopy);
        assert!(output.exists());
        assert_eq!(
            fs::read(&output).expect("read output"),
            fs::read(&src).expect("read src")
        );

        fs::remove_dir_all(&temp_dir).expect("cleanup");
    }
}
