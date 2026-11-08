# Changelog

All notable changes to Oxide are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Copy text from a region without saving the image (Alt+Shift+T), with an option to join lines.
- Region into a new note, a **Today** daily note, and a global shortcut to summon Oxide with a new note (Alt+Shift+N).
- Capture source: each capture records the foreground app and window title (Windows and X11), shown in detail and searchable with `app:`.
- **Never capture** deny list, defaulting to common password managers and private-browsing windows.
- Tags on notes and screenshots, a sidebar tag list, and the `tag:` search filter.
- Manual redaction of selected OCR lines in the original image.
- Oxide logo as README banner, window and tray icon, embedded Windows executable icon, and installer icon.
- Project docs: quick start, architecture, contributing, security policy and this changelog.

### Changed

- Redesigned interface:
  - graphite palette with a copper accent and a refined type scale;
  - motion for view changes, list selection, card hover and status;
  - a modal confirmation dialog.
- New notes focus their title with the placeholder selected.
- Sidebar actions stay pinned while the lists above them scroll.
- Database schema 4 (tags and capture source). Existing databases are backed up before migration; migrated databases cannot be opened by older builds.
- License metadata in `Cargo.toml` now matches the GPL-3.0-or-later `LICENSE`.

### Fixed

- Recent notes with identical titles no longer cause widget ID clashes.

## [0.1.0]

Initial development build: notes with autosave and trash, screenshot capture and import, local OCR, literal full-text search with date and folder filters, Markdown export, global shortcuts, resident mode, and Windows recording exclusion.
