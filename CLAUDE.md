# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Oxide is a local native desktop workspace for notes and OCR-searchable screenshots (Rust, egui/eframe, SQLite), with a Windows-only "recording exclusion" toggle (`WDA_EXCLUDEFROMCAPTURE`). See `README.md` for user-facing behavior and `docs/` for the quick start, architecture, contributing guide, security policy and changelog.

## Commands

Rust 1.95+, edition 2024. CI (`.github/workflows/ci.yml`) runs these on Linux and Windows:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked                    # single test: cargo test -p oxide-core <name>
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked                   # build ALL crates: oxide-app expects oxide-worker beside it
cargo run -p oxide-app --locked
cargo run -p oxide-worker --example pipeline_smoke --locked -- /abs/disposable-dir   # real OCR end-to-end
cargo run -p oxide-core --example search_bench --locked
xvfb-run -a -s "-screen 0 1280x800x24" python3 tests/gui_smoke.py                   # Linux X11 GUI smoke
```

Always use a disposable workspace when running binaries: `--data-dir /abs/path` (both `oxide-app` and `oxide-worker`). Useful flags: app `--no-worker`, `--background`, `--normal`, `--smoke-ui`; worker `--once`, `--index DIR`, `--ocr IMG`, `--search Q`, `--setup-models`. `OXIDE_OFFLINE=1` forbids OCR model downloads. Clippy denies `undocumented_unsafe_blocks` workspace-wide, so every `unsafe` needs a `// SAFETY:` comment.

## Architecture

Cargo workspace of four crates:

- `oxide-core` — shared library, no UI: `Config`/`Paths` (workspace dir descriptor so app and worker agree on config/cache paths), `Store` (rusqlite, schema + migrations in `store.rs`), `Query` (search grammar parser: literal substring terms, `date:`, `after:`, `before:`, `in:`, `tag:`, `app:`), `media` (image load/validate/fingerprint, atomic original replacement, redaction). `atomic_write` is the way to write files.
- `oxide-ocr` — thin wrapper (`Reader`) over `ocrs`/`rten` CPU inference; downloads checksum-pinned ONNX models on first use (names and SHA-256 pins live in `crates/ocr/src/lib.rs`).
- `oxide-worker` — separate OCR/indexing process (and CLI). Watches folders (`notify`), runs OCR, and writes derived data into the same SQLite DB. Isolated from the app so notes and cached search keep working if OCR fails.
- `oxide-app` — the eframe/egui GUI. `app.rs` holds UI state and the capture state machine (`Target`: note, new note, copy text), with views in `app/{shell,notes,screenshots,settings}.rs` and the design system (palette tokens, button styles, motion primitives) in `app/design.rs`; `backend.rs` runs the store on its own thread and is driven through `Request` / `Snapshot` / `Response` channels (UI never touches SQLite directly); `lifecycle.rs` handles the resident/hidden-window lifecycle, global hotkeys, single-instance summon and the worker child process; `platform*.rs` hold OS-specific capture (X11 via x11rb on Linux, Win32 on Windows), foreground-window lookup for capture source and the deny list, and recording-exclusion code. Slow work never runs on the storage thread: copy-text OCR spawns its own thread that runs `oxide-worker --ocr`. `build.rs` embeds `assets/oxide.ico` into the Windows exe.

Key data invariants (from README "Storage and recovery"): notes, screenshot IDs and attachments are canonical; OCR text, FTS rows and thumbnails are derived and rebuildable. Image originals stay on disk as files, never DB blobs. SQLite runs WAL + FULL durability at schema 4; unsupported schema versions are refused (never reset), and migrations in `Store::open` take a no-clobber backup first. A fresh database runs `SCHEMA` then each migration (`SCHEMA_4`), so add schema changes as a new migration, not by editing `SCHEMA`. Search is FTS5 trigram, with user input kept literal (no FTS/SQL injection through the query grammar). OCR confidence is stored as `null` because the engine exposes none — do not fabricate one.

Platform scope: capture source and the deny list need X11 or Windows (Wayland exposes no active window); recording exclusion is Windows-only (Linux reports it unavailable); Wayland capture and macOS adapters are not implemented. Oxide always omits its own UI from its screenshots, independent of the exclusion toggle.

See `docs/ARCHITECTURE.md` for runtime and data-flow detail, `docs/CONTRIBUTING.md` for conventions, and `docs/SECURITY.md` for the threat model. Keep `docs/CHANGELOG.md` updated under `[Unreleased]`. On Linux, keep `--data-dir` paths short: the resident socket path must fit `SUN_LEN`. Windows code can be type-checked from Linux with `--target x86_64-pc-windows-gnu` (mingw-w64).
