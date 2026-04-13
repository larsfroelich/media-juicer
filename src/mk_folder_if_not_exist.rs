use std::path::Path;

use crate::error::{MediaJuicerError, Result};

pub fn ensure_folder_exists(path: &Path) -> Result<bool> {
    if path.exists() {
        if path.is_dir() {
            return Ok(false);
        }
        return Err(MediaJuicerError::InvalidInput(
            "path exists but is not a directory",
        ));
    }

    std::fs::create_dir_all(path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::ensure_folder_exists;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir_name() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "media-juicer-ensure-folder-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn creates_directory_when_missing() {
        let dir = unique_temp_dir_name();
        let created = ensure_folder_exists(&dir).expect("folder creation should succeed");
        assert!(created);
        assert!(dir.is_dir());

        std::fs::remove_dir_all(dir).expect("created directory should be removed");
    }

    #[test]
    fn returns_false_for_existing_directory() {
        let dir = unique_temp_dir_name();
        std::fs::create_dir_all(&dir).expect("directory setup should succeed");

        let created = ensure_folder_exists(&dir).expect("existing directory should succeed");
        assert!(!created);

        std::fs::remove_dir_all(dir).expect("created directory should be removed");
    }
}
