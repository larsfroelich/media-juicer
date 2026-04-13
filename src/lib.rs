pub mod app;
pub mod core;
pub mod io;
pub mod media;
pub mod time;

pub use core::error::{MediaJuicerError, Result};
pub use core::error;

// Backward-compatible re-exports for existing call sites.
pub use core::cli;
pub use core::config;
pub use core::planning;
pub use core::progress;
pub use core::selection;

pub use io::external_apps;
pub use io::fs_discovery;
pub use io::fs_ops;
pub use io::list_files;
pub use io::mk_folder_if_not_exist;

pub use media::image_processing;
pub use media::media_kind;
pub use media::video_processing;

pub use time::fix_dates;
pub(crate) use time::exif_dates;
pub use time::timestamp_policy;
pub use time::timestamps;

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
