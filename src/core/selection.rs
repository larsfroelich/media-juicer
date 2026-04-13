use crate::config::ProcessingMode;
use std::path::{Path, PathBuf};

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

pub fn select_files_for_mode<F>(
    files: &[PathBuf],
    mode: ProcessingMode,
    classifier: F,
) -> Vec<PathBuf>
where
    F: Fn(&Path) -> ClassifiedFile,
{
    files
        .iter()
        .filter(|path| {
            matches!(
                (mode, classifier(path.as_path())),
                (
                    ProcessingMode::All | ProcessingMode::FixDates,
                    ClassifiedFile::Image | ClassifiedFile::Video
                ) | (ProcessingMode::Videos, ClassifiedFile::Video)
                    | (ProcessingMode::Images, ClassifiedFile::Image)
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
    use super::{ClassifiedFile, compute_total_bytes, filter_by_only, select_files_for_mode};
    use crate::config::ProcessingMode;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

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
    fn all_and_fixdates_select_images_and_videos() {
        let files = demo_files();

        let all_selected = select_files_for_mode(&files, ProcessingMode::All, classifier);
        let fixdates_selected = select_files_for_mode(&files, ProcessingMode::FixDates, classifier);

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

        let videos = select_files_for_mode(&files, ProcessingMode::Videos, classifier);
        assert_eq!(videos, vec![PathBuf::from("/media/clip.mp4")]);

        let images = select_files_for_mode(&files, ProcessingMode::Images, classifier);
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
