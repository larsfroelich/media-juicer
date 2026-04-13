use std::path::{Path, PathBuf};

use crate::error::{MediaJuicerError, Result};

pub fn list_files_in_dir(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        return Err(MediaJuicerError::InvalidInput("directory does not exist"));
    }
    if !path.is_dir() {
        return Err(MediaJuicerError::InvalidInput("path is not a directory"));
    }

    let mut files = std::fs::read_dir(path)?
        .collect::<std::result::Result<Vec<_>, std::io::Error>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|entry_path| entry_path.is_file())
        .collect::<Vec<_>>();

    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::list_files_in_dir;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp directory should be created");
        dir
    }

    #[test]
    fn returns_only_files() {
        let dir = unique_temp_dir("media-juicer-list-files");
        let nested_dir = dir.join("nested");
        let first_file = dir.join("a.txt");
        let second_file = dir.join("b.txt");

        std::fs::create_dir_all(&nested_dir).expect("nested dir should be created");
        std::fs::write(&first_file, b"a").expect("first file should be written");
        std::fs::write(&second_file, b"b").expect("second file should be written");

        let files = list_files_in_dir(&dir).expect("directory should be readable");
        assert_eq!(files, vec![first_file, second_file]);

        std::fs::remove_dir_all(dir).expect("temp directory should be removed");
    }

    #[test]
    fn errors_when_dir_is_missing() {
        let missing = std::env::temp_dir().join("media-juicer-missing-dir-123456789");
        let error = list_files_in_dir(&missing).expect_err("missing directory should fail");
        assert_eq!(error.to_string(), "invalid input: directory does not exist");
    }
}
