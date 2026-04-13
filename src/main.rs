fn main() {
    if let Err(error) = media_juicer::cli::parse_args() {
        error.exit();
    }
}
