use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

/// Maps a source path rooted at `src_root` into `out_root`, preserving
/// exactly the relative structure.
pub fn map_to_output_path(
    src_root: &Path,
    out_root: &Path,
    source_path: &Path,
) -> Result<PathBuf, Error> {
    let rel = source_path.strip_prefix(src_root).map_err(|err| {
        Error::new(
            ErrorKind::InvalidInput,
            format!(
                "source path '{}' is not under source root '{}': {err}",
                source_path.display(),
                src_root.display()
            ),
        )
    })?;

    Ok(out_root.join(rel))
}

#[cfg(test)]
mod tests {
    use super::map_to_output_path;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};

    #[test]
    fn output_remap_preserves_relative_structure() {
        let src_root = Path::new("/source");
        let out_root = Path::new("/output");

        let source_file = Path::new("/source/videos/2026/clip.mp4");
        let mapped = map_to_output_path(src_root, out_root, source_file).unwrap();

        assert_eq!(mapped, PathBuf::from("/output/videos/2026/clip.mp4"));
    }

    #[test]
    fn errors_when_source_is_outside_root() {
        let src_root = Path::new("/source");
        let out_root = Path::new("/output");
        let source_file = Path::new("/another/place/clip.mp4");

        let error = map_to_output_path(src_root, out_root, source_file)
            .expect_err("source outside root should error");

        assert_eq!(error.kind(), ErrorKind::InvalidInput);
    }
}
