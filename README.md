# Oxide

A local native workspace for notes and searchable screenshots, with a user-controlled recording-exclusion toggle on Windows.

## Build and run

Requires Rust **1.95 or newer**, a C compiler (bundled SQLite), and a native desktop session. Windows builds use the MSVC toolchain / Visual Studio C++ Build Tools. Linux development builds need an X11 desktop for screenshot capture and native OpenGL/EGL/XKB runtime libraries.

```sh
cargo build --workspace --locked
cargo run -p oxide-app --locked
```

Build the complete workspace so `oxide-worker` is beside `oxide-app`. Notes and cached search remain available if the reader cannot start.

## Everyday workflow

1. Choose a capture directory and optional existing screenshot folders in Settings.
2. Create a note, enter a title/body, and watch the **Saved locally** acknowledgement. Notes autosave after approximately 500 ms of inactivity.
3. Capture a display or select a rectangular region of a frozen display preview. Region coordinates can also be entered using the keyboard. New captures attach to the note active when capture started; library captures can remain unattached.
4. Search notes, screenshots, or **All** results. Open an image to inspect line highlights, copy OCR text, insert selected lines into a note, or attach it to another note.
5. Export a note to Markdown with copied images and relative links. Move notes to persistent note trash; image deletion uses recoverable system trash after confirmation.

Markdown is accepted as text; this development build does not provide a rich-text editor or rendered Markdown preview.

### Recording exclusion

The **Recording exclusion** toggle appears in the header and Settings:

- **On:** applies `WDA_EXCLUDEFROMCAPTURE` to Oxide's owned Windows top-level window before content is shown. Menus/dialogs drawn inside that window inherit its treatment.
- **Off:** removes the affinity, allowing ordinary recording/sharing of Oxide according to the chosen capture source.
- The preference survives restart; switching states does not restart the app or clear notes/search.
- Oxide always omits its own UI from its screenshot originals, independently of this toggle.

Windows capture compatibility is **not yet runtime-validated** for OBS, Zoom, Teams, or Meet. API application is not a guarantee for every recorder/backend. See [compatibility](docs/COMPATIBILITY.md). Linux development builds expose exclusion as unavailable; notes, OCR, and X11 screenshot capture work with it Off. Wayland capture and macOS native adapters are not implemented.

If a treatment change fails, Oxide hides private content. Use the Windows tray action **Recording exclusion Off**, or launch:

```sh
oxide-app --normal
```

This is an explicit request to disable recording exclusion and show the window.

### Shortcuts

| Action | Default |
|---|---|
| Summon / hide | Alt+Shift+O |
| Hide all | Alt+Shift+H |
| Capture region | Alt+Shift+R |
| Capture display | Alt+Shift+D |
| New note | Ctrl+N / Cmd+N |
| Save now | Ctrl+S / Cmd+S |
| Search focus | Ctrl+F / Cmd+F |
| Close panel / hide | Escape |

Global shortcuts are configurable, validated for duplicate assignments, and report registration conflicts. Closing the main window hides the resident app; **Quit** saves pending edits before exiting. Relaunching summons the resident instance. Login startup is optional.

## Local OCR and offline use

The separate Rust reader uses **ocrs + RTen CPU inference**. It downloads two checksum-pinned ONNX model files from the model publisher on first use, and verifies cached hashes on loading. Image pixels and recognized text are never uploaded. The first download requires internet; notes, capture, and cached search work without it.

Prepare models explicitly:

```sh
cargo run -p oxide-worker -- --setup-models
```

After provisioning, set `OXIDE_OFFLINE=1` to prohibit model-download attempts. The models can also be supplied in the workspace's `models/` directory using the exact names and SHA-256 pins in `crates/ocr/src/lib.rs`.

OCR keeps raw strings and normalized line boxes. This engine does not expose recognition confidence; stored confidence is `null`, not a fabricated score. Current models target Latin text; measured multilingual accuracy is not claimed.

### Search

Text terms are literal substrings, and all terms must occur in the same note or screenshot. Screenshot terms may span OCR lines. Quotes, percent signs, underscores, and FTS keywords cannot inject SQL or change the search grammar.

```text
database error
date:2026-10-07
after:2026-10-01 before:2026-11-01
in:"/home/user/Pictures/Screenshots" error
```

Folder filters scope results to screenshots. The **Filters** menu provides date and folder controls. Dates use local calendar boundaries, note update time, and screenshot capture time (file modification time at import/recovery for imported or recovered images). Detail identifies the stored timestamp provenance; older managed records may show unknown legacy provenance. Invalid complete dates produce feedback. Results have Load more controls. Similarity grouping and approximate/semantic search are outside this build.

## Storage and recovery

- SQLite uses WAL, foreign keys, FULL durability, and transactional note/FTS updates.
- Notes, stable screenshot IDs, and attachment relationships are canonical. OCR text and thumbnails are derived and can be rebuilt independently.
- Image originals live in the chosen capture directory or their existing imported locations. They are not stored as database blobs.
- Manual imports remain indexed independently of watched-folder selection. Removed watched folders lose derived searchable content while linked-note metadata is retained.
- Acknowledged note saves survive restart; a still-unacknowledged edit interval can be lost on forced process termination. Save failures retain the editor buffer and expose retry/export.
- Unsupported schema versions are refused without resetting data. Schema 1/2 → schema 3 migration makes a no-clobber SQLite backup first and preserves existing timestamps/relationships.
- Keep screenshot originals and watched roots outside the derived cache. Cache clearing removes numeric `.jpg` thumbnail artifacts; registered originals and other filenames are retained.
- Settings **Back up database** saves a consistent database snapshot. Screenshot originals must be backed up separately. Updates/uninstall do not remove authored data or image directories.
- Local data, OCR, WAL sidecars, and caches are plaintext under the current OS user; recording exclusion is not encryption.

Use an isolated workspace for development/testing:

```sh
oxide-app --data-dir /absolute/path/to/disposable-workspace
oxide-worker --data-dir /absolute/path/to/disposable-workspace --index /absolute/path/to/images
oxide-worker --data-dir /absolute/path/to/disposable-workspace --search "visible phrase"
oxide-worker --data-dir /absolute/path/to/disposable-workspace --ocr /absolute/path/to/image.png
```

`--background` starts the resident app hidden. `--no-worker` runs notes/cached search without starting the OCR child. The workspace directory descriptor keeps app/reader config and cache paths consistent. Logs are local (`reader.log`, stderr, optional `RUST_LOG`).

## Checks

```sh
cargo fmt --all --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run -p oxide-worker --example pipeline_smoke -- /absolute/path/to/disposable-workspace
cargo run -p oxide-core --example search_bench --locked
xvfb-run -a -s "-screen 0 1280x800x24" python3 tests/gui_smoke.py
```

The pipeline check generates a nonprivate fixture, runs actual local OCR, verifies screenshot search/attachments/Markdown export, clears derived caches, and checks saved-note recovery. The X11 GUI check uses real native keyboard events, autosave, saved-pixel verification of own-UI exclusion, hide/resident wake, and restart; it requires libXtst, Xvfb, X11 utilities, and FFmpeg. The isolated synthetic search benchmark covers 10,000 screenshots and 1,000 notes, including short-term and filter-only queries.

Windows installer definition: `packaging/windows/oxide.iss` (Inno Setup 6; build both release binaries first). The CI definition covers native Linux/Windows checks; remote execution and actual recorded-output tests remain release gates.

See [implementation status](docs/IMPLEMENTATION_STATUS.md) and [PRD](PRD.md) for remaining release requirements and measured verification limits.
