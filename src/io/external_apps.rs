use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::OnceLock;

const FFMPEG_ENV: &str = "MEDIA_JUICER_FFMPEG";
const FFPROBE_ENV: &str = "MEDIA_JUICER_FFPROBE";

#[derive(Debug, Clone)]
struct ExternalBinaries {
    ffmpeg: OsString,
    ffprobe: OsString,
}

static BINARIES: OnceLock<ExternalBinaries> = OnceLock::new();

pub fn ffmpeg_binary() -> OsString {
    BINARIES.get_or_init(discover_binaries).ffmpeg.clone()
}

pub fn ffprobe_binary() -> OsString {
    BINARIES.get_or_init(discover_binaries).ffprobe.clone()
}

fn discover_binaries() -> ExternalBinaries {
    ExternalBinaries {
        ffmpeg: discover_binary("ffmpeg", FFMPEG_ENV),
        ffprobe: discover_binary("ffprobe", FFPROBE_ENV),
    }
}

fn discover_binary(command: &str, env_var: &str) -> OsString {
    discover_binary_with_sources(
        command,
        std::env::var_os(env_var),
        std::env::var_os("PATH"),
        common_search_directories(),
    )
}

fn discover_binary_with_sources(
    command: &str,
    env_override: Option<OsString>,
    path_var: Option<OsString>,
    common_dirs: Vec<PathBuf>,
) -> OsString {
    if let Some(configured) = env_override.filter(|value| !value.is_empty()) {
        return configured;
    }

    if let Some(from_path) = find_in_path(command, path_var.as_ref()) {
        return from_path.into_os_string();
    }

    if let Some(from_common_locations) = find_in_directories(command, &common_dirs) {
        return from_common_locations.into_os_string();
    }

    executable_names(command)
        .into_iter()
        .next()
        .map(OsString::from)
        .unwrap_or_else(|| command.into())
}

fn find_in_path(command: &str, path_var: Option<&OsString>) -> Option<PathBuf> {
    let path_var = path_var?;
    let directories = std::env::split_paths(path_var);
    find_in_directories(command, directories.collect::<Vec<_>>().as_slice())
}

fn find_in_directories(command: &str, directories: &[PathBuf]) -> Option<PathBuf> {
    directories.iter().find_map(|directory| {
        executable_names(command)
            .into_iter()
            .map(|name| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn common_search_directories() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .collect();

        let mut expanded = Vec::new();
        for root in roots.drain(..) {
            expanded.push(root.clone());
            expanded.push(root.join("ffmpeg"));
            expanded.push(root.join("ffmpeg").join("bin"));
            expanded.push(root.join("bin"));
            expanded.push(root.join("Microsoft").join("WinGet").join("Links"));
        }
        expanded
    }

    #[cfg(not(windows))]
    {
        [
            "/usr/local/bin",
            "/usr/bin",
            "/opt/homebrew/bin",
            "/opt/local/bin",
            "/snap/bin",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect()
    }
}

fn executable_names(command: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            format!("{command}.exe"),
            format!("{command}.cmd"),
            format!("{command}.bat"),
            command.to_string(),
        ]
    }

    #[cfg(not(windows))]
    {
        vec![command.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::{discover_binary_with_sources, executable_names, find_in_path};
    use std::ffi::OsString;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_file(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, b"binary").expect("failed to write test file");
        path
    }

    #[test]
    fn env_var_override_wins() {
        let temp = TempDir::new().expect("tempdir");
        let custom = create_file(&temp, "custom-ffmpeg");

        let resolved = discover_binary_with_sources(
            "ffmpeg",
            Some(custom.as_os_str().to_os_string()),
            None,
            Vec::new(),
        );

        assert_eq!(resolved, custom.into_os_string());
    }

    #[test]
    fn finds_binary_in_path() {
        let temp = TempDir::new().expect("tempdir");
        let binary_name = executable_names("ffmpeg")
            .into_iter()
            .next()
            .expect("has executable name");
        let binary = create_file(&temp, &binary_name);

        let resolved = find_in_path("ffmpeg", Some(&temp.path().as_os_str().to_os_string()))
            .expect("binary should be found in path");

        assert_eq!(resolved, binary);
    }

    #[test]
    fn falls_back_to_default_command_name() {
        let resolved = discover_binary_with_sources(
            "ffmpeg",
            None,
            Some(OsString::from("/tmp/definitely-not-present")),
            Vec::new(),
        );

        let expected = executable_names("ffmpeg")
            .into_iter()
            .next()
            .expect("at least one executable name");
        assert_eq!(resolved, OsString::from(expected));
    }
}
