mod common;

use common::{MetadataSizeProvider, MockImageBackend, NoopFfmpegExecutor, NoopTimestampProvider};
use media_juicer::app::execute::execute_plan;
use media_juicer::config::{MediaJuicerConfig, ProcessingMode};
use media_juicer::planning::build_processing_plan;
use std::fs;

#[test]
fn fixdates_run_leaves_no_output_tree() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let source_root = tmp.path().join("source");
    fs::create_dir_all(source_root.join("nested")).expect("nested dir");
    fs::write(source_root.join("nested/photo.jpg"), b"image").expect("image file");

    let plan = build_processing_plan(&source_root, ProcessingMode::FixDates, None).expect("plan");
    assert!(!plan.out_folder_path.exists());

    let config = MediaJuicerConfig {
        mode: ProcessingMode::FixDates,
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
    .expect("fixdates execution");

    assert_eq!(summary.progress.processed_files, 1);
    assert!(summary.failures.is_empty());
    assert!(!plan.out_folder_path.exists());
}
