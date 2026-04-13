pub mod fs_discovery;

pub mod media_kind;

pub mod timestamps;

pub mod timestamp_policy;

pub mod video_processing;

pub mod selection;

pub mod fix_dates;

pub mod image_processing;

pub mod fs_ops;

pub mod progress;

pub mod config;

pub mod error;
pub mod list_files;
pub mod mk_folder_if_not_exist;

pub use error::{MediaJuicerError, Result};

/// Returns a short description of the crate's current focus.
pub fn project_summary() -> &'static str {
    "media-juicer organizes and compresses media files."
}

#[cfg(test)]
mod tests {
    use super::project_summary;

    #[test]
    fn summary_mentions_media() {
        assert!(project_summary().contains("media files"));
    }
}
