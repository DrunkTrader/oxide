# Security policy

## Reporting a vulnerability

Please report security issues privately through GitHub's **Report a vulnerability** button on the repository's Security tab (private vulnerability reporting), not in a public issue. Include steps to reproduce, the affected version or commit, and your platform. Do not send real notes, screenshots or databases.

## Supported versions

Oxide is pre-1.0. Fixes land on the latest development branch only.

## Threat model

Oxide keeps everything on your device. Knowing what it does and does not protect helps you decide how to use it.

### What Oxide protects against

- **Network exposure.** Notes, images and recognized text are never uploaded. The only network access is a one-time OCR model download, verified against pinned SHA-256 hashes. With `OXIDE_OFFLINE=1`, no download is attempted.
- **Search injection.** Search input is always literal; it cannot change the query grammar or inject SQL.
- **Accidental capture of sensitive windows.** The **Never capture** list skips captures while a matching app or window title is active (Windows and X11 only).
- **Leaking text in shared screenshots.** Redaction paints opaque black (not blur) over selected lines and removes their text from the search index.
- **Showing Oxide in screen shares or recordings (Windows).** Recording exclusion applies `WDA_EXCLUDEFROMCAPTURE` to Oxide's window.

### What Oxide does not protect against

- **Anyone with access to your user account or disk.** The database, its WAL files, OCR text, thumbnails and originals are stored **unencrypted**. Use full-disk encryption.
- **Malware running as your user.** It can read the data directly, and it can turn off recording exclusion.
- **Every recorder.** Recording exclusion is best-effort. Capture through `PrintWindow`, cameras pointed at the screen, and some recorder backends can still capture Oxide. It has not been validated against OBS, Zoom, Teams or Meet. It is unavailable on Linux and macOS.
- **Imperfect detection.** The deny list matches window names, so a renamed or untitled window can slip through, and Wayland exposes no window names. Redaction only covers the lines you select.

## Recording exclusion: acceptable use

Recording exclusion exists so your private notes stay private while you share your screen. Using it to deceive people, for example to hide assistance during interviews or exams, is not a supported use.
