# media-juicer

`media-juicer` is a small Rust CLI project for organizing and compressing media files.

## Installation

To install `media-juicer`, run the following command:

```bash
cargo install --git https://github.com/larsfroelich/media-juicer
```

## Structure

- `src/lib.rs` contains reusable project logic.
- `src/main.rs` is the CLI entrypoint.
- `legacy/` is reserved for older scripts or migration references.

## Run

```bash
cargo run
```

## Supported input formats

- Images: `.jpg`, `.jpeg`, `.png`, `.bmp`, `.exif`
- Videos: `.mp4`, `.mov`, `.mkv`, `.avi`, `.mts`, `.vob`, `.ts`, `.mpg`, `.mpeg`

`.heic`/`.heif` are currently not supported as image inputs.


## `--only` filtering semantics

`--only` is applied during file selection with predictable matching rules:

- **Default (no path separator, no leading dot):** case-insensitive **exact filename** match.
  - Example: `--only clip.mp4` matches `.../clip.mp4` in any folder, but does not match `my-clip.mp4`.
- **Suffix mode (value starts with `.`):** case-insensitive filename suffix match.
  - Example: `--only .jpg` matches `photo.jpg` and `MIXED.JpG`.
- **Full-path mode (value contains `/` or `\`):** case-insensitive **exact full-path** match.
  - Example: `--only /media/sub/clip.mp4` only matches that full path.

## Development Setup

Run the setup script once per development environment (safe to re-run):

```bash
scripts/setup.sh
```

This script validates required tooling, ensures the pinned Rust toolchain from
`rust-toolchain.toml`, installs `rustfmt` and `clippy`, runs `cargo fetch --locked`,
and exports environment defaults for the current shell:

- `CARGO_TERM_COLOR=always`
- `RUST_BACKTRACE=1`
- `CARGO_INCREMENTAL=0`

After setup, suggested next commands are:

- `scripts/maintain.sh`
- `cargo run`

For maintenance loops and pre-PR validation:

```bash
# Fast checks for iterative local edits
./scripts/maintain-fast.sh

# Full checks before opening a PR
./scripts/maintain.sh
```
