# Quick start

Get Oxide running and capture your first searchable screenshot in about five minutes. For details on any step, see the [README](../README.md).

## 1. Install

**Windows (installer).** Build the release binaries, then compile the installer with [Inno Setup 6](https://jrsoftware.org/isinfo.php):

```sh
cargo build --workspace --release --locked
iscc packaging/windows/oxide.iss
```

Run `dist/Oxide-Setup-0.1.0.exe`. It installs for your user only (no admin rights) and adds Oxide to the Start menu.

**From source (Windows or Linux X11).** Requires Rust 1.95+ and a C compiler:

```sh
cargo build --workspace --locked
cargo run -p oxide-app --locked
```

Build the whole workspace so the OCR reader (`oxide-worker`) sits next to the app.

## 2. First launch

Settings opens on first run.

1. **Storage:** check the capture destination (default: `Pictures/Oxide`) and optionally add folders of existing screenshots to index.
2. **General:** pick Light, Dark or Auto (you can also switch from the header). **Privacy:** on Windows, decide whether **Recording exclusion** should hide Oxide from screen sharing and recorders.
3. Click **Save**.

The first time text is read, Oxide downloads two small OCR model files (about 12 MB, checksum-verified). After that it works fully offline. To download them up front:

```sh
cargo run -p oxide-worker --locked -- --setup-models
```

## 3. Write a note

Press **Ctrl+N** (or click **+ New note**). The title is selected, so just type, then click the body. Notes save automatically; watch for **Saved locally** in the status bar. Write in Markdown and press **Ctrl/Cmd+E** (or the Preview switch) to see it rendered.

Add tags under the title: type a tag and press Enter. Click **Today** in the sidebar for a note named after today's date.

## 4. Capture a screenshot

| Do this | Shortcut |
|---|---|
| Capture a region | Alt+Shift+R |
| Capture the whole display | Alt+Shift+D |
| Copy text from a region (nothing saved) | Alt+Shift+T |
| Summon Oxide with a new note | Alt+Shift+N |
| Show or hide Oxide | Alt+Shift+O |

For a region, drag a rectangle on the frozen preview and release. The screenshot attaches to the note you have open, and its text is read in the background. The **Capture** menu also offers **Region into a new note**.

## 5. Find it again

Press **Ctrl+K** and type any words that appear in your notes or inside your screenshots. Narrow results with filters:

```text
invoice tag:work
date:2026-11-08
app:firefox
in:"/home/me/Pictures/Screenshots"
```

Open a screenshot to see matching lines highlighted, then copy the text, insert it into a note, or attach it to another note.

## 6. Keep sensitive things out

- **Never capture** (Settings → Privacy) skips captures while a password manager or private browsing window is active. Add your own apps or window titles.
- **Redact selected…** in a screenshot's detail view blacks out the lines you select in the original image.
- Data stays on this device but is not encrypted. Use full-disk encryption. See [SECURITY](SECURITY.md).

## Where your data lives

| | Windows | Linux |
|---|---|---|
| Database and models | `%LOCALAPPDATA%\Oxide\Oxide\data` | `~/.local/share/oxide` |
| Settings | `%APPDATA%\Oxide\Oxide\config\config.toml` | `~/.config/oxide/config.toml` |
| Screenshots | your capture destination | your capture destination |

Back up the database from Settings → OCR → **Back up…**. Back up your screenshot folders separately.

## Troubleshooting

- **Text is not being read.** Check the reader status in the status bar, then try Settings → OCR → **Restart**. Make sure `oxide-worker` is next to `oxide-app`.
- **A shortcut does nothing.** Another app may own it. Change it in Settings → Shortcuts.
- **The window disappeared after a recording-exclusion error (Windows).** Use the tray action **Recording exclusion Off**, or run `oxide-app --normal`.
- **Capture fails on Linux.** Oxide needs an X11 session; Wayland capture is not supported yet.
- **Trying things safely.** Run with a throwaway workspace: `oxide-app --data-dir /absolute/path/to/test-workspace`.
