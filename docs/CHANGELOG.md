# Changelog

All notable changes to Oxide are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-11-08

First public release. It includes the development build below plus everything listed here.

### Added

- Copy text from a region without saving the image (Alt+Shift+T), with an option to join lines.
- Region into a new note, a **Today** daily note, and a global shortcut to summon Oxide with a new note (Alt+Shift+N).
- Capture source: each capture records the foreground app and window title (Windows and X11), shown in detail and searchable with `app:`.
- **Never capture** deny list, defaulting to common password managers and private-browsing windows.
- Tags on notes and screenshots, a sidebar tag list, and the `tag:` search filter.
- Manual redaction of selected OCR lines in the original image.
- Rendered Markdown preview for notes: an Edit / Preview switch in the editor (Ctrl/Cmd+E), with headings, lists, task lists (checkboxes can be ticked in Preview), tables, quotes and code. Remote images are never loaded. Note list excerpts show plain text instead of Markdown syntax.
- Inter (variable weight, SIL OFL 1.1) bundled as the UI font, so Markdown bold, headings and titles use real weights. The Markdown renderer's style code is patched for this in `vendor/egui_commonmark_backend` (see `PATCHED.md`).
- Oxide logo as README banner, window and tray icon, embedded Windows executable icon, and installer icon.
- Project docs in `docs/`: quick start, architecture, contributing, security policy and this changelog.
- Release workflow: a `v*` tag builds a `.deb`, `.rpm`, AppImage, Linux tarball, Windows installer and Windows zip, and publishes them with checksums as one GitHub Release.
- Download stats workflow: a daily snapshot of per-file (per-platform) download counts into `downloads.csv` on the `stats` branch.

### Changed

- Redesigned interface in two themes (light and dark, or follow the system), with Apple-style neutrals and one violet accent from the Oxide logo:
  - Light / Dark / Auto switch in the header and in Settings;
  - pill search field with an animated focus ring;
  - Settings as a centred sheet with a sliding tab bar and grouped rows (General, Privacy, Storage, Shortcuts, OCR);
  - screenshot library as an aligned grid of equal tiles, with loading shimmer and staggered entrance;
  - a selection indicator that slides between sidebar items, view transitions, card hover lift, notices as toasts that dismiss themselves, and a modal confirmation dialog.
- New notes focus their title with the placeholder selected.
- Sidebar actions stay pinned while the lists above them scroll.
- Database schema 4 (tags and capture source). Existing databases are backed up before migration; migrated databases cannot be opened by older builds.
- License metadata in `Cargo.toml` now matches the GPL-3.0-or-later `LICENSE`.

### Fixed

- Recent notes with identical titles no longer cause widget ID clashes.
- Windows: saving settings or restarting the reader no longer opens a console window that stays open (the reader and `reg.exe` now start without a console).
- Settings: shortcut fields are full width and the scrollbar sits at the edge of the sheet.
- Screenshot tiles no longer lay out the thumbnail, file name and status side by side.

### Development build (before 0.1.0)

Initial development build: notes with autosave and trash, screenshot capture and import, local OCR, literal full-text search with date and folder filters, Markdown export, global shortcuts, resident mode, and Windows recording exclusion.
