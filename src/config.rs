use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    All,
    Videos,
    Images,
    FixDates,
}

impl FromStr for ProcessingMode {
    type Err = ParseEnumError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "all" => Ok(Self::All),
            "videos" => Ok(Self::Videos),
            "images" => Ok(Self::Images),
            "fixdates" => Ok(Self::FixDates),
            _ => Err(ParseEnumError::new(
                "mode",
                input,
                &["all", "videos", "images", "fixdates"],
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfmpegPreset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
    Placebo,
}

impl FfmpegPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ultrafast => "ultrafast",
            Self::Superfast => "superfast",
            Self::Veryfast => "veryfast",
            Self::Faster => "faster",
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::Slow => "slow",
            Self::Slower => "slower",
            Self::Veryslow => "veryslow",
            Self::Placebo => "placebo",
        }
    }
}

impl fmt::Display for FfmpegPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for FfmpegPreset {
    type Err = ParseEnumError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.trim().to_ascii_lowercase().as_str() {
            "ultrafast" => Ok(Self::Ultrafast),
            "superfast" => Ok(Self::Superfast),
            "veryfast" => Ok(Self::Veryfast),
            "faster" => Ok(Self::Faster),
            "fast" => Ok(Self::Fast),
            "medium" => Ok(Self::Medium),
            "slow" => Ok(Self::Slow),
            "slower" => Ok(Self::Slower),
            "veryslow" => Ok(Self::Veryslow),
            "placebo" => Ok(Self::Placebo),
            _ => Err(ParseEnumError::new(
                "ffmpeg preset",
                input,
                &[
                    "ultrafast",
                    "superfast",
                    "veryfast",
                    "faster",
                    "fast",
                    "medium",
                    "slow",
                    "slower",
                    "veryslow",
                    "placebo",
                ],
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaJuicerConfig {
    pub folder_path: String,
    pub verbose: bool,
    pub mode: ProcessingMode,
    pub replace: Option<String>,
    pub only: Option<String>,
    pub ignore_timestamps: Option<String>,
    pub crf: i32,
    pub ffmpeg_speed: FfmpegPreset,
    pub video_max_pixels: i32,
    pub webpq: i32,
    pub image_max_pixels: i32,
}

impl Default for MediaJuicerConfig {
    fn default() -> Self {
        Self {
            folder_path: String::new(),
            verbose: false,
            mode: ProcessingMode::All,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseEnumError {
    field: &'static str,
    value: String,
    allowed: &'static [&'static str],
}

impl ParseEnumError {
    fn new(field: &'static str, value: &str, allowed: &'static [&'static str]) -> Self {
        Self {
            field,
            value: value.to_string(),
            allowed,
        }
    }
}

impl fmt::Display for ParseEnumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid {} '{}'. Allowed values: {}",
            self.field,
            self.value,
            self.allowed.join(", ")
        )
    }
}

impl std::error::Error for ParseEnumError {}

#[cfg(test)]
mod tests {
    use super::{FfmpegPreset, MediaJuicerConfig, ProcessingMode};
    use std::str::FromStr;

    #[test]
    fn defaults_match_legacy_values() {
        let config = MediaJuicerConfig::default();

        assert_eq!(config.folder_path, "");
        assert!(!config.verbose);
        assert_eq!(config.mode, ProcessingMode::All);
        assert_eq!(config.replace, None);
        assert_eq!(config.only, None);
        assert_eq!(config.ignore_timestamps, None);
        assert_eq!(config.crf, 28);
        assert_eq!(config.ffmpeg_speed, FfmpegPreset::Faster);
        assert_eq!(config.video_max_pixels, 0);
        assert_eq!(config.webpq, 45);
        assert_eq!(config.image_max_pixels, 1600);
    }

    #[test]
    fn valid_mode_values_parse() {
        assert_eq!(ProcessingMode::from_str("all").unwrap(), ProcessingMode::All);
        assert_eq!(ProcessingMode::from_str("videos").unwrap(), ProcessingMode::Videos);
        assert_eq!(ProcessingMode::from_str("images").unwrap(), ProcessingMode::Images);
        assert_eq!(ProcessingMode::from_str("fixdates").unwrap(), ProcessingMode::FixDates);
    }

    #[test]
    fn valid_ffmpeg_presets_parse() {
        assert_eq!(FfmpegPreset::from_str("faster").unwrap(), FfmpegPreset::Faster);
        assert_eq!(FfmpegPreset::from_str("slow").unwrap(), FfmpegPreset::Slow);
        assert_eq!(FfmpegPreset::from_str("placebo").unwrap(), FfmpegPreset::Placebo);
    }

    #[test]
    fn invalid_mode_has_clear_error() {
        let error = ProcessingMode::from_str("audio").unwrap_err();
        let message = error.to_string();

        assert!(message.contains("invalid mode 'audio'"));
        assert!(message.contains("Allowed values: all, videos, images, fixdates"));
    }

    #[test]
    fn invalid_ffmpeg_preset_has_clear_error() {
        let error = FfmpegPreset::from_str("turbo").unwrap_err();
        let message = error.to_string();

        assert!(message.contains("invalid ffmpeg preset 'turbo'"));
        assert!(message.contains("Allowed values: ultrafast, superfast, veryfast"));
    }
}
