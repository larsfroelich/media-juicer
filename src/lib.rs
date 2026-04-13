pub mod selection;

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
