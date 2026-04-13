use std::ffi::OsString;
use std::str::FromStr;

use clap::error::ErrorKind;

use crate::config::{FfmpegPreset, MediaJuicerConfig, ProcessingMode};

const LEGACY_VERSION: &str = "03.00";

pub fn parse_args() -> Result<MediaJuicerConfig, clap::Error> {
    parse_args_from(std::env::args_os())
}

fn command() -> clap::Command {
    clap::Command::new("LFP-Media-Compressor")
        .about("Compress all media in a folder")
        .disable_version_flag(true)
        .arg(
            clap::Arg::new("version")
                .long("version")
                .action(clap::ArgAction::Version)
                .help("Print version"),
        )
        .version(LEGACY_VERSION)
        .arg(
            clap::Arg::new("folder_path")
                .required(true)
                .index(1)
                .help("Source folder path"),
        )
        .arg(
            clap::Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(clap::ArgAction::SetTrue)
                .help("Enable verbose logging"),
        )
        .arg(
            clap::Arg::new("mode")
                .short('m')
                .long("mode")
                .default_value("all")
                .help("Processing mode: all, videos, images, fixdates"),
        )
        .arg(
            clap::Arg::new("replace")
                .long("replace")
                .num_args(0..=1)
                .default_missing_value("true")
                .help("Replace input files with processed output (boolean flag)"),
        )
        .arg(
            clap::Arg::new("only")
                .long("only")
                .value_name("FILENAME")
                .help("Only process files ending with this suffix"),
        )
        .arg(
            clap::Arg::new("ignore_timestamps")
                .long("ignore-timestamps")
                .num_args(0..=1)
                .default_missing_value("true")
                .help("Ignore missing or mismatching file timestamps (boolean flag)"),
        )
        .next_help_heading("Video Options")
        .arg(
            clap::Arg::new("crf")
                .long("crf")
                .default_value("28")
                .help("CRF target (valid range: 0..=51)"),
        )
        .arg(
            clap::Arg::new("ffmpeg_speed")
                .long("ffmpeg-speed")
                .default_value("faster")
                .help("FFmpeg speed profile"),
        )
        .arg(
            clap::Arg::new("video_max_pixels")
                .long("video-max-pixels")
                .default_value("0")
                .help("Max video pixels per dimension; 0 disables resize"),
        )
        .next_help_heading("Image Options")
        .arg(
            clap::Arg::new("webpq")
                .long("webpq")
                .default_value("45")
                .help("WebP image quality (valid range: 0..=100)"),
        )
        .arg(
            clap::Arg::new("image_max_pixels")
                .long("image-max-pixels")
                .default_value("1600")
                .help("Max image pixels per dimension; 0 disables resize"),
        )
}

pub fn parse_args_from<I, T>(args: I) -> Result<MediaJuicerConfig, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let normalized_args = normalize_legacy_args(args);
    let matches = command().try_get_matches_from(normalized_args)?;

    let folder_path = matches
        .get_one::<String>("folder_path")
        .cloned()
        .ok_or_else(|| {
            clap::Error::raw(
                ErrorKind::MissingRequiredArgument,
                "missing required argument: folder_path",
            )
        })?;

    let mode = ProcessingMode::from_str(
        matches
            .get_one::<String>("mode")
            .expect("defaulted by clap"),
    )
    .map_err(|error| clap::Error::raw(ErrorKind::ValueValidation, error.to_string()))?;

    let replace = parse_legacy_bool(matches.get_one::<String>("replace"), "replace")?;
    let ignore_timestamps = parse_legacy_bool(
        matches.get_one::<String>("ignore_timestamps"),
        "ignore-timestamps",
    )?;

    let crf = parse_i32_inclusive(
        matches.get_one::<String>("crf").expect("defaulted by clap"),
        "crf",
        0,
        51,
    )?;

    let ffmpeg_speed = FfmpegPreset::from_str(
        matches
            .get_one::<String>("ffmpeg_speed")
            .expect("defaulted by clap"),
    )
    .map_err(|error| clap::Error::raw(ErrorKind::ValueValidation, error.to_string()))?;

    let video_max_pixels = parse_i32_min(
        matches
            .get_one::<String>("video_max_pixels")
            .expect("defaulted by clap"),
        "video-max-pixels",
        0,
    )?;

    let webpq = parse_i32_inclusive(
        matches
            .get_one::<String>("webpq")
            .expect("defaulted by clap"),
        "webpq",
        0,
        100,
    )?;

    let image_max_pixels = parse_i32_min(
        matches
            .get_one::<String>("image_max_pixels")
            .expect("defaulted by clap"),
        "image-max-pixels",
        0,
    )?;

    Ok(MediaJuicerConfig {
        folder_path,
        verbose: matches.get_flag("verbose"),
        mode,
        replace,
        only: matches.get_one::<String>("only").cloned(),
        ignore_timestamps,
        crf,
        ffmpeg_speed,
        video_max_pixels,
        webpq,
        image_max_pixels,
    })
}

fn normalize_legacy_args<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    args.into_iter()
        .map(|arg| {
            let arg = arg.into();
            if arg == "-crf" { "--crf".into() } else { arg }
        })
        .collect()
}

fn parse_legacy_bool(value: Option<&String>, field: &str) -> Result<bool, clap::Error> {
    let Some(raw) = value else {
        return Ok(false);
    };

    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" => Ok(true),
        "0" | "false" | "no" | "n" | "off" => Ok(false),
        _ => Err(clap::Error::raw(
            ErrorKind::ValueValidation,
            format!(
                "invalid boolean for --{field}: '{raw}'. Allowed values: true/false, 1/0, yes/no"
            ),
        )),
    }
}

fn parse_i32_inclusive(raw: &str, field: &str, min: i32, max: i32) -> Result<i32, clap::Error> {
    let value = raw.parse::<i32>().map_err(|_| {
        clap::Error::raw(
            ErrorKind::ValueValidation,
            format!("invalid value for --{field}: '{raw}' is not an integer"),
        )
    })?;

    if !(min..=max).contains(&value) {
        return Err(clap::Error::raw(
            ErrorKind::ValueValidation,
            format!("invalid value for --{field}: {value} is out of range [{min}, {max}]"),
        ));
    }

    Ok(value)
}

fn parse_i32_min(raw: &str, field: &str, min: i32) -> Result<i32, clap::Error> {
    let value = raw.parse::<i32>().map_err(|_| {
        clap::Error::raw(
            ErrorKind::ValueValidation,
            format!("invalid value for --{field}: '{raw}' is not an integer"),
        )
    })?;

    if value < min {
        return Err(clap::Error::raw(
            ErrorKind::ValueValidation,
            format!("invalid value for --{field}: {value} must be >= {min}"),
        ));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::parse_args_from;
    use crate::config::{FfmpegPreset, ProcessingMode};

    #[test]
    fn parses_defaults_like_legacy() {
        let parsed = parse_args_from(["media-juicer", "/tmp/input"]).unwrap();

        assert_eq!(parsed.folder_path, "/tmp/input");
        assert!(!parsed.verbose);
        assert_eq!(parsed.mode, ProcessingMode::All);
        assert!(!parsed.replace);
        assert_eq!(parsed.only, None);
        assert!(!parsed.ignore_timestamps);
        assert_eq!(parsed.crf, 28);
        assert_eq!(parsed.ffmpeg_speed, FfmpegPreset::Faster);
        assert_eq!(parsed.video_max_pixels, 0);
        assert_eq!(parsed.webpq, 45);
        assert_eq!(parsed.image_max_pixels, 1600);
    }

    #[test]
    fn supports_legacy_single_dash_crf() {
        let parsed = parse_args_from(["media-juicer", "/tmp/input", "-crf", "26"]).unwrap();
        assert_eq!(parsed.crf, 26);
    }

    #[test]
    fn parses_all_supported_options() {
        let parsed = parse_args_from([
            "media-juicer",
            "/tmp/input",
            "--verbose",
            "--mode",
            "videos",
            "--replace",
            "--only",
            "clip.mov",
            "--ignore-timestamps=false",
            "--crf",
            "30",
            "--ffmpeg-speed",
            "slow",
            "--video-max-pixels",
            "1920",
            "--webpq",
            "80",
            "--image-max-pixels",
            "1080",
        ])
        .unwrap();

        assert!(parsed.verbose);
        assert_eq!(parsed.mode, ProcessingMode::Videos);
        assert!(parsed.replace);
        assert_eq!(parsed.only.as_deref(), Some("clip.mov"));
        assert!(!parsed.ignore_timestamps);
        assert_eq!(parsed.crf, 30);
        assert_eq!(parsed.ffmpeg_speed, FfmpegPreset::Slow);
        assert_eq!(parsed.video_max_pixels, 1920);
        assert_eq!(parsed.webpq, 80);
        assert_eq!(parsed.image_max_pixels, 1080);
    }

    #[test]
    fn rejects_invalid_ranges() {
        let error = parse_args_from(["media-juicer", "/tmp/input", "--crf", "99"]).unwrap_err();
        assert!(error.to_string().contains("--crf"));
        assert!(error.to_string().contains("out of range"));
    }

    #[test]
    fn rejects_invalid_boolean_value() {
        let error =
            parse_args_from(["media-juicer", "/tmp/input", "--replace=banana"]).unwrap_err();
        assert!(error.to_string().contains("invalid boolean"));
    }

    #[test]
    fn parses_legacy_boolean_forms() {
        let replace_flag = parse_args_from(["media-juicer", "/tmp/input", "--replace"]).unwrap();
        assert!(replace_flag.replace);

        let replace_true =
            parse_args_from(["media-juicer", "/tmp/input", "--replace=true"]).unwrap();
        assert!(replace_true.replace);

        let replace_one = parse_args_from(["media-juicer", "/tmp/input", "--replace=1"]).unwrap();
        assert!(replace_one.replace);

        let replace_zero = parse_args_from(["media-juicer", "/tmp/input", "--replace=0"]).unwrap();
        assert!(!replace_zero.replace);

        let ignore_yes =
            parse_args_from(["media-juicer", "/tmp/input", "--ignore-timestamps=yes"]).unwrap();
        assert!(ignore_yes.ignore_timestamps);
    }
}
