#![allow(dead_code)]

use media_juicer::image_processing::{BackendImage, ImageBackend, ImageProcessingError};
use media_juicer::timestamps::{
    CreationTimestamps, MediaKind as TimestampMediaKind, TimestampProvider,
};
use media_juicer::video_processing::{FfmpegExecutor, FfmpegRunOutput, FileSizeProvider};
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitStatus;

pub struct MockImageBackend;

impl ImageBackend for MockImageBackend {
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
        fs::write(temp_output_path, b"mock-image")?;
        Ok(())
    }
}

pub fn success_status() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(0)
    }
}

pub struct NoopFfmpegExecutor;

impl FfmpegExecutor for NoopFfmpegExecutor {
    fn run_ffmpeg(&self, _args: &[String]) -> io::Result<FfmpegRunOutput> {
        Ok(FfmpegRunOutput {
            status: success_status(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

pub struct SuccessFfmpegExecutor;

impl FfmpegExecutor for SuccessFfmpegExecutor {
    fn run_ffmpeg(&self, args: &[String]) -> io::Result<FfmpegRunOutput> {
        let output = args.last().expect("ffmpeg output arg should exist");
        fs::write(output, b"encoded-by-mock")?;

        Ok(FfmpegRunOutput {
            status: success_status(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

pub struct MetadataSizeProvider;

impl FileSizeProvider for MetadataSizeProvider {
    fn size_of(&self, path: &Path) -> io::Result<u64> {
        Ok(fs::metadata(path)?.len())
    }
}

pub struct ConstantSizeProvider {
    pub size: u64,
}

impl FileSizeProvider for ConstantSizeProvider {
    fn size_of(&self, _path: &Path) -> io::Result<u64> {
        Ok(self.size)
    }
}

pub struct NoopTimestampProvider;

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
