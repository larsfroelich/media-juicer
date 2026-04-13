use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    All,
    Videos,
    Images,
    Fixdates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeParseError;

impl FromStr for Mode {
    type Err = ModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "all" => Ok(Self::All),
            "videos" => Ok(Self::Videos),
            "images" => Ok(Self::Images),
            "fixdates" => Ok(Self::Fixdates),
            _ => Err(ModeParseError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassifiedFile {
    Image,
    Video,
    Other,
}

pub fn filter_by_only(files: &[PathBuf], only_suffix: Option<&str>) -> Vec<PathBuf> {
    match only_suffix {
        Some(suffix) => files
            .iter()
            .filter(|path| path.to_string_lossy().ends_with(suffix))
            .cloned()
            .collect(),
        None => files.to_vec(),
    }
}

pub fn select_files_for_mode<F>(files: &[PathBuf], mode: Mode, classifier: F) -> Vec<PathBuf>
where
    F: Fn(&Path) -> ClassifiedFile,
{
    files
        .iter()
        .filter(|path| {
            matches!(
                (mode, classifier(path.as_path())),
                (
                    Mode::All | Mode::Fixdates,
                    ClassifiedFile::Image | ClassifiedFile::Video
                ) | (Mode::Videos, ClassifiedFile::Video)
                    | (Mode::Images, ClassifiedFile::Image)
            )
        })
        .cloned()
        .collect()
}

pub fn compute_total_bytes<F>(paths: &[PathBuf], size_provider: F) -> u64
where
    F: Fn(&Path) -> u64,
{
    paths.iter().map(|path| size_provider(path.as_path())).sum()
}

#[cfg(test)]
mod tests {
    use super::{
        ClassifiedFile, Mode, ModeParseError, compute_total_bytes, filter_by_only,
        select_files_for_mode,
    };
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;

    fn demo_files() -> Vec<PathBuf> {
        vec![
            PathBuf::from("/media/photo.jpg"),
            PathBuf::from("/media/clip.mp4"),
            PathBuf::from("/media/notes.txt"),
            PathBuf::from("/media/sub/another.png"),
        ]
    }

    fn classifier(path: &Path) -> ClassifiedFile {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("jpg") | Some("png") => ClassifiedFile::Image,
            Some("mp4") => ClassifiedFile::Video,
            _ => ClassifiedFile::Other,
        }
    }

    #[test]
    fn filter_by_only_matches_legacy_suffix_behavior() {
        let files = demo_files();

        let all_files = filter_by_only(&files, None);
        assert_eq!(all_files, files);

        let filtered = filter_by_only(&files, Some("clip.mp4"));
        assert_eq!(filtered, vec![PathBuf::from("/media/clip.mp4")]);
    }

    #[test]
    fn mode_parsing_rejects_invalid_values() {
        assert_eq!(Mode::from_str("all"), Ok(Mode::All));
        assert_eq!(Mode::from_str("fixdates"), Ok(Mode::Fixdates));
        assert_eq!(Mode::from_str("videos"), Ok(Mode::Videos));
        assert_eq!(Mode::from_str("images"), Ok(Mode::Images));
        assert_eq!(Mode::from_str("invalid"), Err(ModeParseError));
    }

    #[test]
    fn all_and_fixdates_select_images_and_videos() {
        let files = demo_files();

        let all_selected = select_files_for_mode(&files, Mode::All, classifier);
        let fixdates_selected = select_files_for_mode(&files, Mode::Fixdates, classifier);

        let expected = vec![
            PathBuf::from("/media/photo.jpg"),
            PathBuf::from("/media/clip.mp4"),
            PathBuf::from("/media/sub/another.png"),
        ];

        assert_eq!(all_selected, expected);
        assert_eq!(fixdates_selected, expected);
    }

    #[test]
    fn videos_and_images_modes_select_only_their_kind() {
        let files = demo_files();

        let videos = select_files_for_mode(&files, Mode::Videos, classifier);
        assert_eq!(videos, vec![PathBuf::from("/media/clip.mp4")]);

        let images = select_files_for_mode(&files, Mode::Images, classifier);
        assert_eq!(
            images,
            vec![
                PathBuf::from("/media/photo.jpg"),
                PathBuf::from("/media/sub/another.png")
            ]
        );
    }

    #[test]
    fn compute_total_bytes_sums_selected_paths() {
        let paths = vec![
            PathBuf::from("/media/photo.jpg"),
            PathBuf::from("/media/clip.mp4"),
            PathBuf::from("/media/sub/another.png"),
        ];

        let sizes = HashMap::from([
            (PathBuf::from("/media/photo.jpg"), 150_u64),
            (PathBuf::from("/media/clip.mp4"), 1_000_u64),
            (PathBuf::from("/media/sub/another.png"), 350_u64),
        ]);

        let total = compute_total_bytes(&paths, |path| {
            *sizes
                .get(path)
                .expect("size mapping should contain every selected path")
        });

        assert_eq!(total, 1_500);
    }
}
