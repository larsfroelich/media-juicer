mod common;

use common::{MetadataSizeProvider, MockImageBackend, NoopFfmpegExecutor, NoopTimestampProvider};
use media_juicer::app::execute::execute_plan;
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::planning::build_processing_plan;
use std::fs;

#[test]
fn images_pipeline_builds_plan_and_executes_with_mock_backend() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source_root = tmp.path().join("source_tree");
    fs::create_dir_all(source_root.join("nested/deeper")).expect("nested dirs");

    fs::write(source_root.join("top.jpg"), b"img-top").expect("top image");
    fs::write(source_root.join("nested/photo.png"), b"img-nested").expect("nested image");
    fs::write(source_root.join("nested/deeper/clip.mp4"), b"video").expect("video");
    fs::write(source_root.join("notes.txt"), b"other").expect("other");

    let plan = build_processing_plan(&source_root, ProcessingMode::Images, None)
        .expect("plan should build");

    assert_eq!(plan.files.len(), 2, "only image files should be selected");
    assert!(plan.out_folder_path.join("nested").is_dir());
    assert!(plan.out_folder_path.join("nested/deeper").is_dir());

    let config = MediaJuicerConfig {
        mode: ProcessingMode::Images,
        ignore_timestamps: true,
        ..MediaJuicerConfig::default()
    };

    let mut stdout = Vec::new();
    let summary = execute_plan(
        &plan,
        &config,
        &MockImageBackend,
        &NoopFfmpegExecutor,
        &MetadataSizeProvider,
        &NoopTimestampProvider,
        &mut stdout,
    )
    .expect("image execution should succeed");

    assert_eq!(summary.failures.len(), 0);
    assert_eq!(summary.progress.processed_files, 2);

    assert!(plan.out_folder_path.join("top.jpg.webp").exists());
    assert!(plan.out_folder_path.join("nested/photo.png.webp").exists());

    let printed = String::from_utf8(stdout).expect("utf8 progress");
    assert!(printed.contains("Processed 2/2 files"));
}
