# Architecture

This document explains how Oxide is put together: the crates, the processes and threads at runtime, how data flows from a capture to a search result, and the invariants that protect user data. For build commands see the [README](../README.md); for workflow see [CONTRIBUTING](CONTRIBUTING.md).

## Goals that shape the design

- **Local only.** No account, no network except a one-time, checksum-verified OCR model download.
- **Authored data is sacred.** Notes, screenshot identities and attachments must survive crashes, migrations and cache resets. Everything else is derived and rebuildable.
- **The UI never blocks on slow work.** Storage, OCR and capture run off the UI thread.
- **OCR failure is not app failure.** Notes and cached search keep working if the reader cannot start.

## Workspace

```
crates/
  core/    oxide-core    shared library, no UI
  ocr/     oxide-ocr     OCR engine wrapper
  worker/  oxide-worker  background OCR/indexing process and CLI
  app/     oxide-app     native GUI
docs/                    project documentation
assets/                  logo and icons (embedded into the app and installer)
packaging/windows/       Inno Setup installer definition
tests/gui_smoke.py       black-box X11 GUI test
```

| Crate | Responsibility | Key items |
|---|---|---|
| `oxide-core` | Data model, storage, search grammar, config, image I/O | `Store` (`store.rs`), `Query` (`query.rs`), `Config`/`Paths` (`config.rs`), `media.rs`, `atomic_write` |
| `oxide-ocr` | Loads checksum-pinned ONNX models and runs `ocrs` + RTen on CPU | `Reader`, model names and SHA-256 pins in `lib.rs` |
| `oxide-worker` | Watches folders, OCRs pending screenshots, writes derived data; one-shot CLI modes | `main.rs` (`--index`, `--ocr`, `--search`, `--setup-models`) |
| `oxide-app` | egui UI, lifecycle, hotkeys, capture, backend thread | `app.rs`, `app/*.rs`, `backend.rs`, `lifecycle.rs`, `platform*.rs` |

Only `oxide-core` is shared. The app and worker never import each other; they meet at the SQLite database and the workspace directory.

## Runtime model

```
oxide-app process                                   oxide-worker process
+--------------------------------------------+      +------------------------------+
| UI thread (egui)                           |      | watch folders (notify)       |
|   App state, rendering, hotkeys, capture   |      | pick pending screenshot      |
|        | Request            ^ Response     |      | OCR via oxide-ocr Reader     |
|        v                    |              |      | Store::set_ocr               |
| storage thread "oxide-storage"             |      | writes reader.json status    |
|   owns Store (SQLite), media, export       |      +--------------+---------------+
|                                            |                     |
| short-lived "oxide-read-text" threads      |----- spawns -----> oxide-worker --ocr IMAGE
|   copy-text OCR, never blocks saves        |      (one-shot, prints JSON lines)
+--------------------+-----------------------+                     |
                     |                                             |
                     +------------> oxide.db (WAL) <---------------+
```

- **UI thread.** `App` (`crates/app/src/app.rs`) holds view state only. It never opens SQLite. It sends `Request`s and applies `Response`s in `process_replies` every frame.
- **Storage thread.** `Backend` (`backend.rs`) owns the only app-side `Store`. Requests are handled in order, so a queued save always lands before a later load or capture result.
- **Copy-text threads.** `Request::ReadText` is handled off the storage thread. It saves the selection to a temporary PNG and runs `oxide-worker --ocr`, so the UI process never loads OCR models.
- **Worker process.** Started as a child by `lifecycle::Worker`. It holds `reader.lock` so only one indexer runs, and writes `reader.json` so the UI can show its status. If it is missing or crashes, the app keeps working.
- **Resident instance.** `lifecycle::Resident` takes `resident.lock`. A second launch wakes the running instance over a Unix socket (`resident.sock`) on Linux or a token-protected localhost TCP port (`resident.json`) on Windows, then exits.

## Data flow

### Capture to search result

1. A hotkey or menu calls `App::start_capture(region, target)`.
2. `platform::foreground_source()` reads the active window's app and title (Win32 or X11 `_NET_ACTIVE_WINDOW`). If it matches `Config::capture_deny`, the capture stops here.
3. Oxide hides its own window (so it never appears in its own screenshots) and sends `Request::Capture`.
4. For a region, the frozen frame is shown for selection. `App::finish_capture` routes the result by `Target`:
   - `Note`: `Request::SaveCapture`, attached to the active note.
   - `NewNote`: `Request::CaptureToNewNote`, which creates a note and attaches it.
   - `Text`: `Request::ReadText`, which OCRs and copies text, saving nothing.
5. `media::save_capture` writes the PNG atomically; `Store::register_image` records it with `ocr_state='pending'`; `Store::set_source` stores the capture source.
6. The worker picks the pending row, runs OCR, and `Store::set_ocr` replaces `ocr_lines` and the FTS row, guarded by the image digest so a stale result cannot overwrite a newer image.
7. The next `Snapshot` request returns the screenshot in search results.

### Saving a note

`Editor` (`editor.rs`) tracks a generation counter. Edits mark it dirty; after about 500 ms idle the app sends `Request::SaveNote(generation, note)`. `Store::save_note` uses optimistic concurrency (`revision` must match) and updates `notes_fts` in the same transaction. A failure keeps the buffer and offers retry or export.

### Redaction

`Request::Redact` paints opaque black over the selected OCR boxes (`media::redact`), replaces the original with `media::replace_image` (temporary file plus atomic rename), then `Store::redacted` deletes that screenshot's OCR lines and FTS row and re-registers the file. The new digest sets it back to `pending`, so the worker re-reads the redacted image.

## Storage

SQLite in WAL mode with `synchronous=FULL` and foreign keys on. Schema version 4 (`PRAGMA user_version`):

| Table | Kind | Purpose |
|---|---|---|
| `notes` | canonical | title, body, timestamps, `revision`, `deleted_at` (note trash) |
| `screenshots` | canonical | stable id, path, digest, size, capture time and provenance, availability, OCR state, `source_app`, `source_title` |
| `note_screenshots` | canonical | attachments with position |
| `note_tags`, `shot_tags` | canonical | normalized tags |
| `ocr_lines` | derived | per-line text and normalized boxes; `confidence` is NULL because `ocrs` exposes none |
| `notes_fts`, `screenshots_fts` | derived | FTS5 trigram indexes |
| `changes` | bookkeeping | single revision counter the UI polls to detect changes |

Image originals are files in the capture directory or the user's own folders, never database blobs. Thumbnails live in the cache directory and are rebuildable.

**Migrations** live in `Store::open`. Rules:

- Before migrating an existing database, take a no-clobber backup (`oxide-before-schema-N-<ms>.db`).
- Refuse unknown or future versions without touching data.
- A fresh database runs the base schema, then every migration, so there is one source of truth for each change.

## Search

`Query::parse` turns input into literal terms plus filters: `date:`, `after:`, `before:`, `in:"folder"` (screenshots only), `tag:` (notes and screenshots), `app:` (screenshots only). `search_clause` in `store.rs` builds SQL with bound parameters only:

- Terms of three or more characters use FTS5 `MATCH` with each term quoted.
- Shorter terms use `LIKE` with escaped wildcards.
- User input can never change the grammar or inject SQL.

## UI structure

- `app.rs`: `App` state, request and response handling, capture state machine, modals.
- `app/shell.rs`: header, sidebar (workspace, Today, tags, collections, recent), status bar.
- `app/notes.rs`: note list and editor, tag row.
- `app/screenshots.rs`: capture menu, library grid, detail view, OCR line selection, redaction.
- `app/settings.rs`: settings window and filters menu.
- `app/design.rs`: the design system. `Palette` (light and dark tokens), `apply_theme`, button styles (`primary`, `destructive`, `quiet`), form building blocks (`form_group`, `form_row`, `segmented`, `search_field`), and motion primitives (`entrance`, `stagger`, `hover_card`, `list_row` + `nav_indicator`, `shimmer`, `toast`, `status_dot`). Add visual constants here, not inline.

Settings is a modal sheet inside the main window rather than a separate native window: recording exclusion protects Oxide's own window, and a second window would need its own treatment or could appear in a screen share.

## Platform matrix

| Capability | Windows | Linux X11 | Linux Wayland | macOS |
|---|---|---|---|---|
| Capture | GDI `BitBlt` | `x11rb` `GetImage` | not implemented | not implemented |
| Capture source and deny list | Win32 foreground window | EWMH atoms | unavailable | not implemented |
| Recording exclusion | `WDA_EXCLUDEFROMCAPTURE` | unavailable | unavailable | not implemented |
| Tray icon | yes | no | no | no |
| Login startup | registry Run key | XDG autostart | XDG autostart | no |
| Embedded exe icon | `build.rs` + `winresource` | n/a | n/a | n/a |

Recording exclusion is best-effort: `PrintWindow`-based capture ignores it, and local code can turn it off. It is a privacy convenience, not a security boundary (see [SECURITY](SECURITY.md)).
