use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ProcessingMode;
use crate::error::{MediaJuicerError, Result};
use crate::fs_discovery::{list_files, list_folders};
use crate::list_files::map_to_output_path;
use crate::media_kind::{MediaKind, classify_path};
use crate::mk_folder_if_not_exist::ensure_folder_exists;
use crate::selection::{
    ClassifiedFile, compute_total_bytes, filter_by_only, select_files_for_mode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub source_path: PathBuf,
    pub media_kind: MediaKind,
    pub output_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingPlan {
    pub source_root: PathBuf,
    pub out_folder_path: PathBuf,
    pub files: Vec<PlannedFile>,
    pub total_bytes_to_process: u64,
}

pub fn build_processing_plan(
    source_root: &Path,
    mode: ProcessingMode,
    only_suffix: Option<&str>,
) -> Result<ProcessingPlan> {
    let out_folder_path = if mode == Mode::Fixdates {
        source_root.to_path_buf()
    } else {
        out_folder_for_source(source_root)?
    };

    if mode != Mode::Fixdates {
        ensure_folder_exists(&out_folder_path)?;

        for src_folder in list_folders(source_root)? {
            let mirrored_folder = map_to_output_path(source_root, &out_folder_path, &src_folder)?;
            ensure_folder_exists(&mirrored_folder)?;
        }
    }

    let discovered_files = list_files(source_root)?;
    let only_filtered = filter_by_only(&discovered_files, only_suffix);

    let selected_files =
        select_files_for_mode(&only_filtered, mode, |path| match classify_path(path) {
            MediaKind::Image => ClassifiedFile::Image,
            MediaKind::Video => ClassifiedFile::Video,
            MediaKind::Other => ClassifiedFile::Other,
        });

    let total_bytes_to_process = compute_total_bytes(&selected_files, |path| {
        fs::metadata(path)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    });

    let mut files = Vec::with_capacity(selected_files.len());
    for source_path in selected_files {
        let media_kind = classify_path(&source_path);
        let output_path = map_to_output_path(source_root, &out_folder_path, &source_path)?;
        let size_bytes = fs::metadata(&source_path)?.len();

        files.push(PlannedFile {
            source_path,
            media_kind,
            output_path,
            size_bytes,
        });
    }

    Ok(ProcessingPlan {
        source_root: source_root.to_path_buf(),
        out_folder_path,
        files,
        total_bytes_to_process,
    })
}

fn out_folder_for_source(source_root: &Path) -> Result<PathBuf> {
    let normalized = source_root;

    let folder_name = normalized
        .file_name()
        .ok_or(MediaJuicerError::InvalidInput(
            "source root must have a terminal folder name",
        ))?;

    let out_folder_name = format!("{}_compressed", folder_name.to_string_lossy());

    Ok(normalized
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(out_folder_name))
}

#[cfg(test)]
mod tests {
    use super::{build_processing_plan, out_folder_for_source};
    use crate::config::ProcessingMode;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&path).expect("temp dir should be creatable");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn out_folder_matches_legacy_suffix_behavior() {
        let source_root = Path::new("/tmp/input-folder");
        let out = out_folder_for_source(source_root).unwrap();
        assert_eq!(out, PathBuf::from("/tmp/input-folder_compressed"));
    }

    #[test]
    fn builds_plan_with_mode_and_only_filtering_and_mirrored_folders() {
        let tmp = TempDir::new("planning-module");
        let source_root = tmp.path().join("source");
        fs::create_dir_all(source_root.join("nested/deeper")).unwrap();

        let keep_video = source_root.join("nested/deeper/clip.mp4");
        let keep_image = source_root.join("photo.jpg");
        let skip_other = source_root.join("nested/readme.txt");

        fs::write(&keep_video, vec![0_u8; 10]).unwrap();
        fs::write(&keep_image, vec![0_u8; 30]).unwrap();
        fs::write(&skip_other, vec![1_u8; 50]).unwrap();

        let plan = build_processing_plan(&source_root, ProcessingMode::All, Some(".jpg")).unwrap();

        assert_eq!(plan.total_bytes_to_process, 30);
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].source_path, keep_image);
        assert_eq!(
            plan.files[0].output_path,
            plan.out_folder_path.join("photo.jpg")
        );

        assert!(plan.out_folder_path.exists());
        assert!(plan.out_folder_path.join("nested").is_dir());
        assert!(plan.out_folder_path.join("nested/deeper").is_dir());
    }

    #[test]
    fn mode_selection_applies_before_total_bytes() {
        let tmp = TempDir::new("planning-mode-selection");
        let source_root = tmp.path().join("source");
        fs::create_dir_all(&source_root).unwrap();

        let video = source_root.join("video.mp4");
        let image = source_root.join("image.jpg");
        fs::write(&video, vec![0_u8; 111]).unwrap();
        fs::write(&image, vec![0_u8; 222]).unwrap();

        let plan = build_processing_plan(&source_root, ProcessingMode::Videos, None).unwrap();

        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].source_path, video);
        assert_eq!(plan.total_bytes_to_process, 111);
    }

    #[test]
    fn fixdates_mode_uses_source_root_and_skips_compressed_tree_creation() {
        let tmp = TempDir::new("planning-fixdates");
        let source_root = tmp.path().join("source");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(source_root.join("clip.mp4"), b"video").unwrap();

        let plan = build_processing_plan(&source_root, Mode::Fixdates, None).unwrap();

        assert_eq!(plan.out_folder_path, source_root);
        assert!(!tmp.path().join("source_compressed").is_dir());
    }

    #[test]
    fn fixdates_does_not_create_output_tree() {
        let tmp = TempDir::new("planning-fixdates-no-output");
        let source_root = tmp.path().join("source");
        fs::create_dir_all(source_root.join("nested")).unwrap();
        fs::write(source_root.join("nested/photo.jpg"), vec![0_u8; 10]).unwrap();

        let plan = build_processing_plan(&source_root, Mode::Fixdates, None).unwrap();

        assert_eq!(plan.files.len(), 1);
        assert!(!plan.out_folder_path.exists());
        assert!(!plan.out_folder_path.join("nested").exists());
    }

    #[test]
    fn image_video_and_all_modes_still_create_output_tree() {
        let modes = [Mode::Images, Mode::Videos, Mode::All];

        for mode in modes {
            let tmp = TempDir::new("planning-mode-output-tree");
            let source_root = tmp.path().join("source");
            fs::create_dir_all(source_root.join("nested")).unwrap();
            fs::write(source_root.join("nested/photo.jpg"), vec![0_u8; 10]).unwrap();
            fs::write(source_root.join("nested/video.mp4"), vec![0_u8; 10]).unwrap();

            let plan = build_processing_plan(&source_root, mode, None).unwrap();

            assert!(plan.out_folder_path.exists());
            assert!(plan.out_folder_path.join("nested").is_dir());
        }
    }
}
