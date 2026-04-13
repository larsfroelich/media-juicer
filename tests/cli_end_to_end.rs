use assert_cmd::prelude::*;
use predicates::str::contains;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn write_executable_script(path: &Path, content: &str) {
    fs::write(path, content).expect("script should be writable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("chmod");
    }
}

fn create_mock_tools(tmp: &TempDir) -> (PathBuf, PathBuf) {
    let ffmpeg = tmp.path().join("ffmpeg-mock.sh");
    write_executable_script(
        &ffmpeg,
        r#"#!/usr/bin/env sh
set -eu
out=""
for arg in "$@"; do
  out="$arg"
done
printf "mock-encoded" > "$out"
"#,
    );

    let ffprobe = tmp.path().join("ffprobe-mock.sh");
    write_executable_script(
        &ffprobe,
        "#!/usr/bin/env sh\necho '2024-01-02T03:04:05.000000Z'\n",
    );

    (ffmpeg, ffprobe)
}

fn media_juicer_cmd() -> Command {
    Command::cargo_bin("media-juicer").expect("binary should build")
}

#[test]
fn invalid_input_directory_returns_exit_code_2_and_invalid_input_error() {
    let missing = std::env::temp_dir().join(format!(
        "media-juicer-missing-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    media_juicer_cmd()
        .arg(&missing)
        .assert()
        .code(2)
        .stderr(contains(
            "invalid input: source folder does not exist or is not a directory",
        ));
}

#[test]
fn mode_fixdates_does_not_create_compressed_tree_and_prints_progress_summary() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input");
    fs::create_dir_all(&input).expect("mkdir input");
    fs::write(input.join("clip.mp4"), b"video").expect("write source");

    let (ffmpeg, ffprobe) = create_mock_tools(&tmp);

    let assert = media_juicer_cmd()
        .arg(&input)
        .arg("--mode")
        .arg("fixdates")
        .env("MEDIA_JUICER_FFMPEG", ffmpeg)
        .env("MEDIA_JUICER_FFPROBE", ffprobe)
        .assert()
        .success()
        .stdout(contains("Processed 1/1 files"))
        .stdout(contains("Processed a total of 1 files."));

    let _ = assert;

    let compressed = input.parent().expect("parent").join("input_compressed");
    assert!(
        !compressed.exists(),
        "fixdates mode must not create _compressed tree"
    );
}

#[test]
fn mode_videos_replace_rewrites_source_to_mp4_and_keeps_output() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let input = tmp.path().join("input");
    fs::create_dir_all(&input).expect("mkdir input");

    let source = input.join("clip.mov");
    fs::write(&source, b"original-source").expect("source");

    let compressed = tmp.path().join("input_compressed");
    fs::create_dir_all(&compressed).expect("mkdir compressed");
    let output = compressed.join("clip.mov.mp4");
    fs::write(&output, b"existing-output").expect("existing output");

    let (ffmpeg, ffprobe) = create_mock_tools(&tmp);

    media_juicer_cmd()
        .arg(&input)
        .arg("--mode")
        .arg("videos")
        .arg("--replace")
        .arg("true")
        .env("MEDIA_JUICER_FFMPEG", ffmpeg)
        .env("MEDIA_JUICER_FFPROBE", ffprobe)
        .assert()
        .success()
        .stdout(contains("Processed 1/1 files"))
        .stdout(contains("Processed a total of 1 files."));

    assert!(!source.exists(), "original .mov source should be removed");
    let rewritten = input.join("clip.mov.mp4");
    assert!(
        rewritten.exists(),
        "source should be rewritten to .mp4 path"
    );
    assert_eq!(
        fs::read(&rewritten).expect("rewritten bytes"),
        b"existing-output"
    );
    assert_eq!(fs::read(&output).expect("output bytes"), b"existing-output");
}

#[test]
fn boolean_parsing_matrix_for_replace_and_ignore_timestamps() {
    let truthy = ["true", "1", "yes"];
    let falsy = ["false", "0", "no"];

    for value in truthy {
        let tmp = tempfile::tempdir().expect("tempdir truthy replace");
        let input = tmp.path().join("input");
        fs::create_dir_all(&input).expect("mkdir input");

        let source = input.join("clip.mov");
        fs::write(&source, b"original-source").expect("source");

        let compressed = tmp.path().join("input_compressed");
        fs::create_dir_all(&compressed).expect("mkdir compressed");
        fs::write(compressed.join("clip.mov.mp4"), b"existing-output").expect("output");

        let (ffmpeg, ffprobe) = create_mock_tools(&tmp);

        media_juicer_cmd()
            .arg(&input)
            .arg("--mode")
            .arg("videos")
            .arg("--replace")
            .arg(value)
            .env("MEDIA_JUICER_FFMPEG", ffmpeg)
            .env("MEDIA_JUICER_FFPROBE", ffprobe)
            .assert()
            .success()
            .stdout(contains("Processed 1/1 files"))
            .stdout(contains("Processed a total of 1 files."));

        assert!(
            input.join("clip.mov.mp4").exists(),
            "--replace={value} should be parsed as true"
        );
        assert!(
            !source.exists(),
            "source should be removed for truthy --replace={value}"
        );
    }

    for value in falsy {
        let tmp = tempfile::tempdir().expect("tempdir falsy replace");
        let input = tmp.path().join("input");
        fs::create_dir_all(&input).expect("mkdir input");

        let source = input.join("clip.mov");
        fs::write(&source, b"original-source").expect("source");

        let compressed = tmp.path().join("input_compressed");
        fs::create_dir_all(&compressed).expect("mkdir compressed");
        fs::write(compressed.join("clip.mov.mp4"), b"existing-output").expect("output");

        let (ffmpeg, ffprobe) = create_mock_tools(&tmp);

        media_juicer_cmd()
            .arg(&input)
            .arg("--mode")
            .arg("videos")
            .arg("--replace")
            .arg(value)
            .env("MEDIA_JUICER_FFMPEG", ffmpeg)
            .env("MEDIA_JUICER_FFPROBE", ffprobe)
            .assert()
            .success()
            .stdout(contains("Processed 1/1 files"))
            .stdout(contains("Processed a total of 1 files."));

        assert!(
            source.exists(),
            "source should remain for falsy --replace={value}"
        );
        assert!(
            !input.join("clip.mov.mp4").exists(),
            "--replace={value} should be parsed as false"
        );
    }

    for value in truthy {
        let tmp = tempfile::tempdir().expect("tempdir truthy ignore");
        let input = tmp.path().join("input");
        fs::create_dir_all(&input).expect("mkdir input");
        fs::write(input.join("photo.jpg"), b"not-a-real-jpeg").expect("image");

        let (ffmpeg, ffprobe) = create_mock_tools(&tmp);

        media_juicer_cmd()
            .arg(&input)
            .arg("--mode")
            .arg("fixdates")
            .arg("--ignore-timestamps")
            .arg(value)
            .env("MEDIA_JUICER_FFMPEG", ffmpeg)
            .env("MEDIA_JUICER_FFPROBE", ffprobe)
            .assert()
            .success()
            .stdout(contains("Processed 1/1 files"))
            .stdout(contains("Processed a total of 1 files."));
    }

    for value in falsy {
        let tmp = tempfile::tempdir().expect("tempdir falsy ignore");
        let input = tmp.path().join("input");
        fs::create_dir_all(&input).expect("mkdir input");
        fs::write(input.join("photo.jpg"), b"not-a-real-jpeg").expect("image");

        let (ffmpeg, ffprobe) = create_mock_tools(&tmp);

        media_juicer_cmd()
            .arg(&input)
            .arg("--mode")
            .arg("fixdates")
            .arg("--ignore-timestamps")
            .arg(value)
            .env("MEDIA_JUICER_FFMPEG", ffmpeg)
            .env("MEDIA_JUICER_FFPROBE", ffprobe)
            .assert()
            .code(1)
            .stdout(contains("Processed 1/1 files"))
            .stdout(contains("Processed a total of 1 files."));
    }
}
