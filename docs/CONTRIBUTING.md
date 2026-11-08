# Contributing to Oxide

Thanks for helping. This guide covers setup, the checks every change must pass, and the conventions that keep user data safe. Read [ARCHITECTURE](ARCHITECTURE.md) first for the big picture.

## Setup

- Rust 1.95 or newer (edition 2024) and a C compiler (SQLite is bundled).
- **Linux:** an X11 desktop, plus OpenGL/EGL/XKB runtime libraries. For the GUI test, install `xvfb`, `libxtst6`, X11 utilities and `ffmpeg`.
- **Windows:** the MSVC toolchain (Visual Studio C++ Build Tools with the Windows SDK, which provides `rc.exe` for the embedded icon).

```sh
cargo build --workspace --locked
cargo run -p oxide-app --locked -- --data-dir /absolute/path/to/disposable-workspace
```

Build the whole workspace so `oxide-worker` sits next to `oxide-app`.

**Always use a disposable `--data-dir`** when running locally, so you never touch your real notes. Keep the path short on Linux: the resident socket path must fit `SUN_LEN` (about 108 bytes). To prepare OCR models once:

```sh
cargo run -p oxide-worker --locked -- --data-dir /path/to/workspace --setup-models
```

## Checks

CI (`.github/workflows/ci.yml`) runs these on Linux and Windows. Run them before opening a pull request:

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p oxide-worker --example pipeline_smoke --locked -- /absolute/path/to/disposable-workspace
xvfb-run -a -s "-screen 0 1280x800x24" python3 tests/gui_smoke.py
```

Run a single test with `cargo test -p oxide-core <test_name>`.

Changing Windows code on Linux? Type-check it with the GNU target:

```sh
rustup target add x86_64-pc-windows-gnu   # needs mingw-w64
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
```

## Conventions

### Data safety (non-negotiable)

- Notes, screenshot ids, attachments and tags are canonical. Never delete or reset them to fix derived state; OCR text, FTS rows and thumbnails can always be rebuilt.
- Write files with `oxide_core::config::atomic_write` or a temporary file plus atomic rename. Never write an original in place.
- **Schema changes** go in `Store::open` as a new migration:
  - bump `PRAGMA user_version`;
  - keep the pre-migration backup;
  - refuse unknown versions;
  - add a test that migrates an older database and checks data survives.
- Keep user input literal in SQL: bound parameters only, never string-built conditions from user text.
- Operations that change originals (trash, redaction) need an explicit confirmation in the UI.

### Code

- The UI thread never touches SQLite. Add a `Request` and handle it in `backend.rs`.
- Slow work (OCR, external processes) must not run on the storage thread. See how `Request::ReadText` spawns its own thread.
- Every `unsafe` block needs a `// SAFETY:` comment; clippy denies `undocumented_unsafe_blocks`.
- Put colors, spacing and motion in `crates/app/src/app/design.rs`, not inline.
- Prefer the standard library and existing dependencies. Justify any new dependency in the pull request.
- Platform code lives in `platform_linux.rs` and `platform_windows.rs` behind `cfg`. Return `None` or a clear error where a platform cannot support a feature; never fake it.
- Comment the why, not the what.

### Tests

- New logic in `oxide-core` (store, query, media, config) gets a unit test next to the code.
- Bug fixes include a test that fails without the fix.

## Commits and pull requests

- Use [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`.
- One logical change per pull request. Describe user-visible behavior, any schema or config change, and how you tested it (including which platforms).
- Update the [README](../README.md), [ARCHITECTURE](ARCHITECTURE.md) and [CHANGELOG](CHANGELOG.md) when behavior changes.

## Reporting bugs and security issues

Open a GitHub issue with steps to reproduce, your OS and display server, and relevant lines from `reader.log`. Never attach your real database or screenshots. For security issues, follow [SECURITY](SECURITY.md) instead of opening a public issue.

By contributing you agree your work is licensed under the project's [GPL-3.0-or-later](../LICENSE) license.
