use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use filetime::{FileTime, set_file_mtime};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

use crate::exif_dates;

const TIMESTAMP_MISMATCH_THRESHOLD: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct ImageJob {
    pub source_path: PathBuf,
    pub output_path: PathBuf,
    pub quality: u8,
    pub max_pixels: u32,
    pub ignore_timestamps: bool,
}

#[derive(Debug, Clone)]
pub struct BackendImage {
    pub source_timestamp: Option<SystemTime>,
    pub decoded: DynamicImage,
}

impl BackendImage {
    pub fn new(decoded: DynamicImage, source_timestamp: Option<SystemTime>) -> Self {
        Self {
            source_timestamp,
            decoded,
        }
    }
}

pub trait ImageBackend {
    fn open(&self, source_path: &Path) -> Result<BackendImage, ImageProcessingError>;
    fn resize(&self, image: &mut BackendImage, max_pixels: u32)
    -> Result<(), ImageProcessingError>;
    fn save(
        &self,
        image: &BackendImage,
        temp_output_path: &Path,
        quality: u8,
    ) -> Result<(), ImageProcessingError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemImageBackend;

impl ImageBackend for SystemImageBackend {
    fn open(&self, source_path: &Path) -> Result<BackendImage, ImageProcessingError> {
        let decoded = image::open(source_path).map_err(|error| {
            ImageProcessingError::Backend(format!("failed to decode image: {error}"))
        })?;

        let source_timestamp = read_exif_timestamp(source_path).or_else(|| {
            fs::metadata(source_path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
        });

        Ok(BackendImage::new(decoded, source_timestamp))
    }

    fn resize(
        &self,
        image: &mut BackendImage,
        max_pixels: u32,
    ) -> Result<(), ImageProcessingError> {
        let (width, height) = image.decoded.dimensions();
        if width <= max_pixels && height <= max_pixels {
            return Ok(());
        }

        image.decoded = image
            .decoded
            .resize(max_pixels, max_pixels, FilterType::Lanczos3);
        Ok(())
    }

    fn save(
        &self,
        image: &BackendImage,
        temp_output_path: &Path,
        quality: u8,
    ) -> Result<(), ImageProcessingError> {
        let rgba = image.decoded.to_rgba8();
        let encoder = webp::Encoder::from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
        let encoded = encoder.encode(quality as f32);
        let encoded_bytes: &[u8] = encoded.as_ref();
        fs::write(temp_output_path, encoded_bytes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessOutcome {
    SkippedExistingOutput,
    AbortedTimestampMismatch,
    Processed,
}

#[derive(Debug)]
pub enum ImageProcessingError {
    Io(std::io::Error),
    Backend(String),
}

impl Display for ImageProcessingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Backend(err) => write!(f, "Backend error: {err}"),
        }
    }
}

impl std::error::Error for ImageProcessingError {}

impl From<std::io::Error> for ImageProcessingError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn resolve_webp_output_path(output_path: &Path) -> PathBuf {
    crate::media_kind::image_output_path(output_path)
}

pub fn temp_output_path_for(output_path: &Path) -> PathBuf {
    let output_path_str = output_path.to_string_lossy();
    PathBuf::from(format!("{output_path_str}.tmp.webp"))
}

pub fn process_image_job<B: ImageBackend>(
    job: &ImageJob,
    backend: &B,
) -> Result<ProcessOutcome, ImageProcessingError> {
    let output_path = resolve_webp_output_path(&job.output_path);
    let temp_output_path = temp_output_path_for(&output_path);

    if output_path.exists() {
        return Ok(ProcessOutcome::SkippedExistingOutput);
    }

    if temp_output_path.exists() {
        fs::remove_file(&temp_output_path)?;
    }

    let source_metadata = fs::metadata(&job.source_path)?;
    let source_modified = source_metadata.modified()?;

    let mut image = backend.open(&job.source_path)?;

    if !job.ignore_timestamps
        && timestamps_mismatch(
            image.source_timestamp,
            Some(source_modified),
            TIMESTAMP_MISMATCH_THRESHOLD,
        )
    {
        return Ok(ProcessOutcome::AbortedTimestampMismatch);
    }

    if job.max_pixels > 0 {
        backend.resize(&mut image, job.max_pixels)?;
    }

    backend.save(&image, &temp_output_path, job.quality)?;

    let output_timestamp = image.source_timestamp.unwrap_or(source_modified);
    set_file_mtime(
        &temp_output_path,
        FileTime::from_system_time(output_timestamp),
    )?;
    fs::rename(&temp_output_path, &output_path)?;

    Ok(ProcessOutcome::Processed)
}

fn read_exif_timestamp(source_path: &Path) -> Option<SystemTime> {
    exif_dates::read_exif_timestamp(source_path, None).map(SystemTime::from)
}

fn timestamps_mismatch(
    lhs: Option<SystemTime>,
    rhs: Option<SystemTime>,
    threshold: Duration,
) -> bool {
    match (lhs, rhs) {
        (Some(a), Some(b)) => {
            let diff = match a.duration_since(b) {
                Ok(duration) => duration,
                Err(err) => err.duration(),
            };
            diff > threshold
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;
    use std::time::Duration;

    use tempfile::tempdir;

    #[derive(Default)]
    struct MockState {
        open_calls: usize,
        resize_calls: usize,
        save_calls: usize,
    }

    struct MockBackend {
        state: Rc<RefCell<MockState>>,
        timestamp_to_return: Option<SystemTime>,
    }

    impl MockBackend {
        fn with_timestamp(
            timestamp_to_return: Option<SystemTime>,
        ) -> (Self, Rc<RefCell<MockState>>) {
            let state = Rc::new(RefCell::new(MockState::default()));
            (
                Self {
                    state: Rc::clone(&state),
                    timestamp_to_return,
                },
                state,
            )
        }
    }

    impl ImageBackend for MockBackend {
        fn open(&self, _source_path: &Path) -> Result<BackendImage, ImageProcessingError> {
            self.state.borrow_mut().open_calls += 1;
            Ok(BackendImage::new(
                DynamicImage::new_rgba8(1, 1),
                self.timestamp_to_return,
            ))
        }

        fn resize(
            &self,
            _image: &mut BackendImage,
            _max_pixels: u32,
        ) -> Result<(), ImageProcessingError> {
            self.state.borrow_mut().resize_calls += 1;
            Ok(())
        }

        fn save(
            &self,
            _image: &BackendImage,
            temp_output_path: &Path,
            _quality: u8,
        ) -> Result<(), ImageProcessingError> {
            self.state.borrow_mut().save_calls += 1;
            fs::write(temp_output_path, b"mock-webp")?;
            Ok(())
        }
    }

    fn build_job(source_path: &Path, output_path: &Path, ignore_timestamps: bool) -> ImageJob {
        ImageJob {
            source_path: source_path.to_path_buf(),
            output_path: output_path.to_path_buf(),
            quality: 85,
            max_pixels: 2_000,
            ignore_timestamps,
        }
    }

    #[test]
    fn skip_when_output_exists() {
        let dir = tempdir().expect("tempdir");
        let source_path = dir.path().join("input.jpg");
        let output_path = dir.path().join("output.webp");
        fs::write(&source_path, b"source").expect("source write");
        fs::write(&output_path, b"existing").expect("output write");

        let (backend, state) = MockBackend::with_timestamp(None);
        let job = build_job(&source_path, &output_path, false);

        let outcome = process_image_job(&job, &backend).expect("process ok");

        assert_eq!(outcome, ProcessOutcome::SkippedExistingOutput);
        let state = state.borrow();
        assert_eq!(state.open_calls, 0);
        assert_eq!(state.save_calls, 0);
    }

    #[test]
    fn delete_stale_temp_output() {
        let dir = tempdir().expect("tempdir");
        let source_path = dir.path().join("input.jpg");
        let output_path = dir.path().join("output");
        let resolved_output_path = resolve_webp_output_path(&output_path);
        let temp_output_path = temp_output_path_for(&resolved_output_path);

        fs::write(&source_path, b"source").expect("source write");
        fs::write(&temp_output_path, b"stale-temp").expect("temp write");

        let source_modified = fs::metadata(&source_path)
            .expect("source metadata")
            .modified()
            .expect("source modified");

        let (backend, state) = MockBackend::with_timestamp(Some(source_modified));
        let job = build_job(&source_path, &output_path, false);

        let outcome = process_image_job(&job, &backend).expect("process ok");

        assert_eq!(outcome, ProcessOutcome::Processed);
        assert!(resolved_output_path.exists());
        assert!(!temp_output_path.exists());
        assert_eq!(state.borrow().save_calls, 1);
    }

    #[test]
    fn abort_on_mismatch_when_not_ignored() {
        let dir = tempdir().expect("tempdir");
        let source_path = dir.path().join("input.jpg");
        let output_path = dir.path().join("output");
        fs::write(&source_path, b"source").expect("source write");

        let source_modified = fs::metadata(&source_path)
            .expect("source metadata")
            .modified()
            .expect("source modified");
        let mismatched_timestamp = source_modified + Duration::from_secs(2 * 24 * 60 * 60);

        let (backend, state) = MockBackend::with_timestamp(Some(mismatched_timestamp));
        let job = build_job(&source_path, &output_path, false);

        let outcome = process_image_job(&job, &backend).expect("process ok");

        assert_eq!(outcome, ProcessOutcome::AbortedTimestampMismatch);
        assert!(!resolve_webp_output_path(&output_path).exists());
        assert_eq!(state.borrow().save_calls, 0);
    }

    #[test]
    fn continue_on_mismatch_when_ignored() {
        let dir = tempdir().expect("tempdir");
        let source_path = dir.path().join("input.jpg");
        let output_path = dir.path().join("output");
        fs::write(&source_path, b"source").expect("source write");

        let source_modified = fs::metadata(&source_path)
            .expect("source metadata")
            .modified()
            .expect("source modified");
        let mismatched_timestamp = source_modified + Duration::from_secs(2 * 24 * 60 * 60);

        let (backend, state) = MockBackend::with_timestamp(Some(mismatched_timestamp));
        let job = build_job(&source_path, &output_path, true);

        let outcome = process_image_job(&job, &backend).expect("process ok");

        assert_eq!(outcome, ProcessOutcome::Processed);
        assert!(resolve_webp_output_path(&output_path).exists());
        assert_eq!(state.borrow().save_calls, 1);
    }
}
