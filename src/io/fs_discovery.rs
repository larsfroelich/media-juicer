use std::fs;
use std::io::Error;
use std::path::{Path, PathBuf};
use std::{collections::HashSet, io::ErrorKind};

/// Recursively lists every file under `root`.
///
/// Results are sorted for deterministic behavior.
pub fn list_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    visit_descendants(root, &mut |path| {
        if path.is_file() {
            files.push(path.to_path_buf());
        }
    })?;

    files.sort();
    Ok(files)
}

/// Recursively lists every folder under `root` (excluding `root`).
///
/// Results are sorted for deterministic behavior.
pub fn list_folders(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut folders = Vec::new();
    visit_descendants(root, &mut |path| {
        if path.is_dir() {
            folders.push(path.to_path_buf());
        }
    })?;

    folders.sort();
    Ok(folders)
}

fn visit_descendants(root: &Path, on_entry: &mut impl FnMut(&Path)) -> Result<(), Error> {
    let mut dirs = vec![root.to_path_buf()];
    let mut visited_dirs = HashSet::new();
    visited_dirs.insert(fs::canonicalize(root)?);

    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            on_entry(&path);

            if path.is_dir() {
                // Symlink policy: follow directory symlinks only if they resolve to
                // a canonical directory target that has not been visited yet.
                let canonical = match fs::canonicalize(&path) {
                    Ok(canonical) => canonical,
                    Err(err) if err.kind() == ErrorKind::NotFound => continue,
                    Err(err) => return Err(err),
                };

                if visited_dirs.insert(canonical.clone()) {
                    dirs.push(canonical);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{list_files, list_folders};
    use std::env;
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
            let dir = env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
            fs::create_dir_all(&dir).expect("temp dir should be creatable");

            Self { path: dir }
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
    fn discovers_nested_files_and_folders() {
        let tmp = TempDir::new("fs-discovery-nested");
        let root = tmp.path();

        let dir_a = root.join("a");
        let dir_b = root.join("a/b");
        let dir_c = root.join("c");
        fs::create_dir_all(&dir_b).unwrap();
        fs::create_dir_all(&dir_c).unwrap();

        let file_1 = root.join("top.txt");
        let file_2 = root.join("a/inner.txt");
        let file_3 = root.join("a/b/deep.txt");
        fs::write(&file_1, b"top").unwrap();
        fs::write(&file_2, b"inner").unwrap();
        fs::write(&file_3, b"deep").unwrap();

        let files = list_files(root).unwrap();
        assert_eq!(files, vec![file_3.clone(), file_2.clone(), file_1.clone()]);

        let folders = list_folders(root).unwrap();
        assert_eq!(folders, vec![dir_a.clone(), dir_b.clone(), dir_c.clone()]);
    }

    #[test]
    fn handles_empty_dirs() {
        let tmp = TempDir::new("fs-discovery-empty");
        let root = tmp.path();

        let empty = root.join("empty");
        let nested_empty = root.join("empty/nested");
        fs::create_dir_all(&nested_empty).unwrap();

        let files = list_files(root).unwrap();
        assert!(files.is_empty());

        let folders = list_folders(root).unwrap();
        assert_eq!(folders, vec![empty, nested_empty]);
    }

    #[test]
    #[cfg(unix)]
    fn follows_symlink_dirs_once_without_loops() {
        use std::os::unix::fs::symlink;

        let tmp = TempDir::new("fs-discovery-symlink-loop-unix");
        let root = tmp.path();

        let dir_a = root.join("a");
        let dir_b = dir_a.join("b");
        fs::create_dir_all(&dir_b).unwrap();

        let file = dir_b.join("file.txt");
        fs::write(&file, b"hello").unwrap();

        let loop_link = dir_b.join("back-to-a");
        symlink(&dir_a, &loop_link).unwrap();

        let alias = root.join("alias-a");
        symlink(&dir_a, &alias).unwrap();

        let files = list_files(root).unwrap();
        assert_eq!(files, vec![file.clone()]);

        let folders = list_folders(root).unwrap();
        assert_eq!(folders, vec![dir_a, dir_b, loop_link, alias]);
    }

    #[test]
    #[cfg(windows)]
    fn follows_symlink_dirs_once_without_loops() {
        use std::os::windows::fs::symlink_dir;

        let tmp = TempDir::new("fs-discovery-symlink-loop-windows");
        let root = tmp.path();

        let dir_a = root.join("a");
        let dir_b = dir_a.join("b");
        fs::create_dir_all(&dir_b).unwrap();

        let file = dir_b.join("file.txt");
        fs::write(&file, b"hello").unwrap();

        let loop_link = dir_b.join("back-to-a");
        if let Err(err) = symlink_dir(&dir_a, &loop_link) {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                eprintln!("Skipping test: creating directory symlink requires privileges.");
                return;
            }
            panic!("failed to create loop symlink: {err}");
        }

        let alias = root.join("alias-a");
        if let Err(err) = symlink_dir(&dir_a, &alias) {
            if err.kind() == std::io::ErrorKind::PermissionDenied {
                eprintln!("Skipping test: creating directory symlink requires privileges.");
                return;
            }
            panic!("failed to create alias symlink: {err}");
        }

        let files = list_files(root).unwrap();
        assert_eq!(files, vec![file.clone()]);

        let folders = list_folders(root).unwrap();
        assert_eq!(folders, vec![dir_a, dir_b, loop_link, alias]);
    }
}
