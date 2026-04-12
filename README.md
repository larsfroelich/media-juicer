# media-juicer

`media-juicer` is a small Rust CLI project for organizing and compressing media files.
The current setup is intentionally minimal: one crate with a library used by the binary.

## Structure

- `src/lib.rs` contains reusable project logic.
- `src/main.rs` is the CLI entrypoint.
- `legacy/` is reserved for older scripts or migration references.

## Run

```bash
cargo run
```
