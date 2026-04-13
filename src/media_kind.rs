use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Other,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".mov", ".mkv", ".avi", ".mts", ".vob", ".ts", ".mpg", ".mpeg",
];

const IMAGE_EXTENSIONS: &[&str] = &[".jpg", ".jpeg", ".png", ".bmp", ".exif"];

pub fn classify_path(path: impl AsRef<Path>) -> MediaKind {
    let path_lower = path.as_ref().to_string_lossy().to_lowercase();

    if VIDEO_EXTENSIONS.iter().any(|ext| path_lower.ends_with(ext)) {
        MediaKind::Video
    } else if IMAGE_EXTENSIONS.iter().any(|ext| path_lower.ends_with(ext)) {
        MediaKind::Image
    } else {
        MediaKind::Other
    }
}

pub fn image_output_path(path: impl AsRef<Path>) -> PathBuf {
    with_output_extension(path, ".webp")
}

pub fn video_output_path(path: impl AsRef<Path>) -> PathBuf {
    with_output_extension(path, ".mp4")
}

fn with_output_extension(path: impl AsRef<Path>, extension: &str) -> PathBuf {
    let input = path.as_ref();
    let input_string = input.to_string_lossy();

    if input_string.to_lowercase().ends_with(extension) {
        input.to_path_buf()
    } else {
        PathBuf::from(format!("{input_string}{extension}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{MediaKind, classify_path, image_output_path, video_output_path};
    use std::path::PathBuf;

    #[test]
    fn classify_path_uses_legacy_extensions_case_insensitively() {
        struct Case {
            input: &'static str,
            expected: MediaKind,
        }

        let cases = [
            Case {
                input: "clip.mp4",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.MOV",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.mKv",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.avi",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.MTS",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.vob",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.Ts",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.mpg",
                expected: MediaKind::Video,
            },
            Case {
                input: "clip.MPEG",
                expected: MediaKind::Video,
            },
            Case {
                input: "photo.jpg",
                expected: MediaKind::Image,
            },
            Case {
                input: "photo.JPEG",
                expected: MediaKind::Image,
            },
            Case {
                input: "photo.pNg",
                expected: MediaKind::Image,
            },
            Case {
                input: "photo.BMP",
                expected: MediaKind::Image,
            },
            Case {
                input: "photo.exif",
                expected: MediaKind::Image,
            },
            Case {
                input: "archive.zip",
                expected: MediaKind::Other,
            },
        ];

        for case in cases {
            assert_eq!(
                classify_path(case.input),
                case.expected,
                "wrong media kind for {}",
                case.input
            );
        }
    }

    #[test]
    fn image_output_path_ensures_webp_extension() {
        struct Case {
            input: &'static str,
            expected: &'static str,
        }

        let cases = [
            Case {
                input: "photo.jpg",
                expected: "photo.jpg.webp",
            },
            Case {
                input: "photo.webp",
                expected: "photo.webp",
            },
            Case {
                input: "photo.WEBP",
                expected: "photo.WEBP",
            },
            Case {
                input: "photo",
                expected: "photo.webp",
            },
        ];

        for case in cases {
            assert_eq!(image_output_path(case.input), PathBuf::from(case.expected));
        }
    }

    #[test]
    fn video_output_path_ensures_mp4_extension() {
        struct Case {
            input: &'static str,
            expected: &'static str,
        }

        let cases = [
            Case {
                input: "clip.mov",
                expected: "clip.mov.mp4",
            },
            Case {
                input: "clip.mp4",
                expected: "clip.mp4",
            },
            Case {
                input: "clip.MP4",
                expected: "clip.MP4",
            },
            Case {
                input: "clip",
                expected: "clip.mp4",
            },
        ];

        for case in cases {
            assert_eq!(video_output_path(case.input), PathBuf::from(case.expected));
        }
    }
}
