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

## Supported input formats

- Images: `.jpg`, `.jpeg`, `.png`, `.bmp`, `.exif`
- Videos: `.mp4`, `.mov`, `.mkv`, `.avi`, `.mts`, `.vob`, `.ts`, `.mpg`, `.mpeg`

`.heic`/`.heif` are currently not supported as image inputs.

## Codex setup

Run the setup script once per development environment (safe to re-run):

```bash
scripts/setup-rust-codex.sh
```

This script validates required tooling, ensures the pinned Rust toolchain from
`rust-toolchain.toml`, installs `rustfmt` and `clippy`, runs `cargo fetch --locked`,
and exports Codex-friendly defaults for the current shell:

- `CARGO_TERM_COLOR=always`
- `RUST_BACKTRACE=1`
- `CARGO_INCREMENTAL=0` (optional deterministic CI-like behavior)

After setup, suggested next commands are:

- `scripts/maintain-rust-codex.sh`
- `cargo run`


For maintenance loops and pre-PR validation:

```bash
# Fast checks for iterative local/Codex edits
./scripts/maintain-rust-codex-fast.sh

# Full checks before opening a PR
./scripts/maintain-rust-codex.sh
```

Use `maintain-rust-codex-fast.sh` while iterating on code when you want quick feedback.
Run `maintain-rust-codex.sh` before creating a PR for comprehensive validation.
