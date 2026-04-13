use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Controls which extension should be used for the final output path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalExtensionPolicy {
    /// Keep the original full path, including its current extension.
    KeepOriginalPath,
    /// Use the output file extension on top of the original file stem.
    MatchOutputExtension,
}

/// Remove a file if it exists.
///
/// This operation is idempotent: missing files are treated as success.
pub fn remove_if_exists(path: impl AsRef<Path>) -> io::Result<()> {
    match fs::remove_file(path.as_ref()) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// Rename `from` to `to` using the platform's atomic rename primitive.
pub fn atomic_rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    fs::rename(from, to)
}

/// Copy `src` to `dst`, then set accessed/modified times on `dst`.
pub fn copy_then_preserve_times(
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    time: SystemTime,
) -> io::Result<u64> {
    let bytes_copied = fs::copy(src, &dst)?;
    let file = fs::OpenOptions::new().write(true).open(dst)?;
    let times = fs::FileTimes::new().set_accessed(time).set_modified(time);
    file.set_times(times)?;
    Ok(bytes_copied)
}

/// Replace the original file with the processed output based on extension policy.
pub fn replace_original_with_output(
    original: impl AsRef<Path>,
    output: impl AsRef<Path>,
    final_ext_policy: FinalExtensionPolicy,
) -> io::Result<PathBuf> {
    let original = original.as_ref();
    let output = output.as_ref();

    let final_path = match final_ext_policy {
        FinalExtensionPolicy::KeepOriginalPath => original.to_path_buf(),
        FinalExtensionPolicy::MatchOutputExtension => {
            let mut final_path = original.to_path_buf();
            if let Some(output_ext) = output.extension() {
                final_path.set_extension(output_ext);
            }
            final_path
        }
    };

    if output != final_path {
        remove_if_exists(&final_path)?;
        atomic_rename(output, &final_path)?;
    }

    if original != final_path {
        remove_if_exists(original)?;
    }

    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::{
        FinalExtensionPolicy, atomic_rename, copy_then_preserve_times, remove_if_exists,
        replace_original_with_output,
    };
    use std::fs;
    use std::io;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> io::Result<Self> {
            let mut path = std::env::temp_dir();
            let unique = format!(
                "media_juicer_test_{}_{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time should be after epoch")
                    .as_nanos()
            );
            path.push(unique);
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn join(&self, file_name: &str) -> PathBuf {
            self.path.join(file_name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn remove_if_exists_is_idempotent() {
        let temp_dir = TempDir::new().expect("should create temp dir");
        let target = temp_dir.join("to_delete.tmp");
        fs::write(&target, b"data").expect("should create file");

        remove_if_exists(&target).expect("first delete should succeed");
        remove_if_exists(&target).expect("second delete should also succeed");

        assert!(!target.exists());
    }

    #[test]
    fn rename_and_copy_with_time_preservation_work() {
        let temp_dir = TempDir::new().expect("should create temp dir");

        let rename_from = temp_dir.join("rename_from.bin");
        let rename_to = temp_dir.join("rename_to.bin");
        fs::write(&rename_from, b"rename me").expect("should write rename source");

        atomic_rename(&rename_from, &rename_to).expect("rename should succeed");
        assert!(!rename_from.exists());
        assert_eq!(
            fs::read(&rename_to).expect("should read renamed file"),
            b"rename me"
        );

        let copy_src = temp_dir.join("copy_src.bin");
        let copy_dst = temp_dir.join("copy_dst.bin");
        fs::write(&copy_src, b"copy me").expect("should write copy source");
        let expected_time = UNIX_EPOCH + Duration::from_secs(12_345);

        copy_then_preserve_times(&copy_src, &copy_dst, expected_time)
            .expect("copy with time preservation should succeed");

        assert_eq!(
            fs::read(&copy_dst).expect("should read copied file"),
            b"copy me"
        );
        let modified = fs::metadata(&copy_dst)
            .expect("should read metadata")
            .modified()
            .expect("should read modified time");
        assert_eq!(modified, expected_time);
    }

    #[test]
    fn replace_original_uses_output_extension_when_different() {
        let temp_dir = TempDir::new().expect("should create temp dir");

        let original = temp_dir.join("clip.mov");
        let output = temp_dir.join("clip.mp4");

        fs::write(&original, b"old contents").expect("should write original");
        fs::write(&output, b"new contents").expect("should write output");

        let final_path = replace_original_with_output(
            &original,
            &output,
            FinalExtensionPolicy::MatchOutputExtension,
        )
        .expect("replacement should succeed");

        let expected_final = temp_dir.join("clip.mp4");
        assert_eq!(final_path, expected_final);
        assert!(!original.exists());
        assert!(expected_final.exists());
        assert_eq!(
            fs::read(&expected_final).expect("should read final file"),
            b"new contents"
        );
    }
}
