use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
};

use anyhow::{Context, Result};
use eframe::egui;
use image::DynamicImage;
use oxide_core::{
    Line, Note, NoteSummary, Screenshot, WorkerStatus,
    config::{Config, Paths},
    media,
    query::Query,
    store::Store,
};

use crate::platform::{self, Display};

pub enum Request {
    Snapshot {
        generation: u64,
        query: String,
        trash: bool,
        note: Option<i64>,
        shot: Option<i64>,
        limit: usize,
    },
    NewNote,
    LoadNote(i64),
    SaveNote(u64, Note),
    Duplicate(i64),
    NoteTrash(i64, bool),
    Attach(i64, i64),
    Detach(i64, i64),
    Import(PathBuf, Option<i64>),
    Paste(Option<i64>, PathBuf),
    Capture(Display),
    SaveCapture(DynamicImage, Option<i64>, PathBuf),
    Image(i64, bool),
    Reindex(Option<i64>),
    TrashShots(Vec<i64>),
    Export(i64, PathBuf),
    ExportBuffer(Note, PathBuf),
    Config(Config),
    ClearCache,
    Backup(PathBuf),
    Shutdown,
}

pub struct Snapshot {
    pub generation: u64,
    pub revision: i64,
    pub notes: Vec<NoteSummary>,
    pub shots: Vec<Screenshot>,
    pub attachments: Vec<Screenshot>,
    pub lines: Vec<Line>,
    pub linked: Vec<NoteSummary>,
    pub status: WorkerStatus,
    pub note: Option<oxide_core::Note>,
}

pub enum Response {
    Snapshot(Snapshot),
    Note(Note),
    Saved(u64, Note),
    SaveFailed(String),
    Captured(DynamicImage),
    CaptureFailed(String),
    Image {
        id: i64,
        full: bool,
        digest: String,
        updated_at: i64,
        size: [usize; 2],
        rgba: Vec<u8>,
        lines: Vec<Line>,
    },
    ImageFailed(i64, bool, String),
    Configured(Config),
    ConfigFailed(String),
    Changed(String),
    Error(String),
    StorageFailed(String),
}

pub struct Backend {
    sender: SyncSender<Request>,
    pub receiver: Receiver<Response>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Backend {
    pub fn start(paths: Paths, context: egui::Context) -> Result<Self> {
        let (sender, requests) = mpsc::sync_channel(32);
        let (replies, receiver) = mpsc::channel();
        let handle = thread::Builder::new()
            .name("oxide-storage".into())
            .spawn(move || {
                let emit = |response| {
                    if replies.send(response).is_ok() {
                        context.request_repaint();
                    }
                };
                let mut store = match Store::open(&paths.database()) {
                    Ok(store) => store,
                    Err(error) => {
                        emit(Response::StorageFailed(format!("{error:#}")));
                        return;
                    }
                };
                for request in requests {
                    if matches!(request, Request::Shutdown) {
                        break;
                    }
                    let response = handle_request(request, &mut store, &paths);
                    emit(response);
                }
            })?;
        Ok(Self {
            sender,
            receiver,
            handle: Some(handle),
        })
    }
    pub fn send(&self, request: Request) -> Result<()> {
        self.sender
            .try_send(request)
            .map_err(|error| anyhow::anyhow!("Background operation could not be queued: {error}"))
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        if self.sender.send(Request::Shutdown).is_err() {
            log::debug!("Storage worker was already stopped");
        }
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            log::error!("Storage worker terminated unexpectedly");
        }
    }
}

fn handle_request(request: Request, store: &mut Store, paths: &Paths) -> Response {
    match request {
        Request::SaveNote(generation, note) => match store.save_note(&note) {
            Ok(note) => Response::Saved(generation, note),
            Err(e) => Response::SaveFailed(format!("{e:#}")),
        },
        Request::Capture(display) => match platform::capture(&display) {
            Ok(image) => Response::Captured(image),
            Err(e) => Response::CaptureFailed(format!("{e:#}")),
        },
        Request::Image(id, full) => match decode_image(store, paths, id, full) {
            Ok(response) => response,
            Err(e) => Response::ImageFailed(id, full, format!("{e:#}")),
        },
        Request::Config(config) => match save_config(paths, &config) {
            Ok(()) => Response::Configured(config),
            Err(e) => Response::ConfigFailed(format!("{e:#}")),
        },
        other => match operation(other, store, paths) {
            Ok(response) => response,
            Err(e) => Response::Error(format!("{e:#}")),
        },
    }
}

fn operation(request: Request, store: &mut Store, paths: &Paths) -> Result<Response> {
    match request {
        Request::Snapshot {
            generation,
            query,
            trash,
            note,
            shot,
            limit,
        } => {
            let query = Query::parse(&query)?;
            let status = worker_status(paths);
            let attachments = if let Some(id) = note {
                store.attachments(id)?
            } else {
                Vec::new()
            };
            let lines = if let Some(id) = shot {
                store.lines(id)?
            } else {
                Vec::new()
            };
            let linked = if let Some(id) = shot {
                store.linked_notes(id)?
            } else {
                Vec::new()
            };
            Ok(Response::Snapshot(Snapshot {
                generation,
                revision: store.revision()?,
                notes: store.notes(&query, trash, limit, 0)?,
                shots: store.screenshots(&query, limit, 0)?,
                attachments,
                lines,
                linked,
                status,
                note: if let Some(id) = note {
                    store.note(id)?
                } else {
                    None
                },
            }))
        }
        Request::NewNote => Ok(Response::Note(store.new_note()?)),
        Request::LoadNote(id) => Ok(Response::Note(
            store.note(id)?.context("Note no longer exists")?,
        )),
        Request::Duplicate(id) => Ok(Response::Note(store.duplicate_note(id)?)),
        Request::NoteTrash(id, trash) => {
            store.set_note_trashed(id, trash)?;
            Ok(Response::Changed(
                if trash {
                    "Note moved to note trash"
                } else {
                    "Note restored"
                }
                .into(),
            ))
        }
        Request::Attach(note, shot) => {
            store.attach(note, shot)?;
            Ok(Response::Changed("Screenshot attached".into()))
        }
        Request::Detach(note, shot) => {
            store.detach(note, shot)?;
            Ok(Response::Changed(
                "Attachment removed; original retained".into(),
            ))
        }
        Request::Import(path, note) => {
            paths.ensure_original_location(&path)?;
            let shot = store.register_import(&path)?;
            anyhow::ensure!(
                shot.ocr_state != "invalid",
                "Image is invalid: {}",
                shot.ocr_error.as_deref().unwrap_or("Cannot decode image")
            );
            if let Some(id) = note {
                store.attach(id, shot.id)?;
            }
            if let Err(error) = media::thumbnail(paths, shot.id, &media::load_image(&path)?, &path)
            {
                return Ok(Response::Changed(format!(
                    "Image imported and attachment committed; thumbnail will be retried: {error:#}"
                )));
            }
            Ok(Response::Changed("Image imported; OCR queued".into()))
        }
        Request::Paste(note, destination) => {
            let mut clipboard = arboard::Clipboard::new().context("Cannot access the clipboard")?;
            let image = clipboard
                .get_image()
                .context("Clipboard does not contain an image")?;
            let width = u32::try_from(image.width)?;
            let height = u32::try_from(image.height)?;
            media::validate_dimensions(width, height)?;
            let rgba = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())
                .context("Clipboard image buffer is invalid")?;
            save_capture(
                store,
                paths,
                DynamicImage::ImageRgba8(rgba),
                note,
                &destination,
            )
        }
        Request::SaveCapture(image, note, destination) => {
            save_capture(store, paths, image, note, &destination)
        }
        Request::Reindex(id) => {
            store.reindex(id)?;
            Ok(Response::Changed("OCR queued for retry".into()))
        }
        Request::TrashShots(ids) => {
            let mut successes = 0;
            let mut errors = Vec::new();
            for id in ids {
                match store.screenshot(id)?.context("Selected screenshot no longer exists").and_then(|shot| {
                    trash::delete(&shot.path).context("Cannot move original to recoverable system trash")?;
                    store.trash_image(id).context("Original moved to trash, but library update failed; reconciliation will repair it")
                }) { Ok(()) => successes += 1, Err(e) => errors.push(format!("Screenshot {id}: {e:#}")) }
            }
            if !errors.is_empty() {
                anyhow::bail!(
                    "Moved {successes} originals to system trash; {} failures. {}",
                    errors.len(),
                    errors.join("; ")
                );
            }
            Ok(Response::Changed(format!(
                "Moved {successes} originals to system trash"
            )))
        }
        Request::Export(id, destination) => {
            media::export_note(store, id, &destination)?;
            Ok(Response::Changed(format!(
                "Exported note to {}",
                destination.display()
            )))
        }
        Request::ExportBuffer(note, destination) => {
            oxide_core::config::atomic_write(
                &destination,
                format!("# {}\n\n{}\n", note.title, note.body).as_bytes(),
            )?;
            Ok(Response::Changed("Unsaved editing buffer exported".into()))
        }
        Request::ClearCache => {
            let thumbs = paths.cache.join("thumbs");
            if thumbs.exists() {
                for entry in fs::read_dir(thumbs)? {
                    let entry = entry?;
                    let path = entry.path();
                    let generated = path.extension().is_some_and(|ext| ext == "jpg")
                        && path
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .is_some_and(|stem| stem.parse::<i64>().is_ok());
                    if entry.file_type()?.is_file() && generated && !store.is_original(&path)? {
                        fs::remove_file(path)?;
                    }
                }
            }
            store.clear_derived()?;
            Ok(Response::Changed(
                "Derived caches cleared; notes, attachments, and originals retained".into(),
            ))
        }
        Request::Backup(path) => {
            store.backup(&path)?;
            Ok(Response::Changed(
                "Database backup saved; managed image originals are separate".into(),
            ))
        }
        Request::Shutdown => Ok(Response::Changed("Storage closed".into())),
        Request::SaveNote(_, _)
        | Request::Capture(_)
        | Request::Image(_, _)
        | Request::Config(_) => unreachable!("Handled before operation dispatch"),
    }
}

fn save_capture(
    store: &mut Store,
    paths: &Paths,
    image: DynamicImage,
    note: Option<i64>,
    destination: &Path,
) -> Result<Response> {
    fs::create_dir_all(destination).context("Cannot create capture directory")?;
    paths.ensure_original_location(destination)?;
    let file = media::save_capture(&image, destination)?;
    let shot = store.register_image(&file,true,Some(oxide_core::now())).with_context(||format!("Original saved at {}, but registration failed; it will be recovered from the capture directory",file.display()))?;
    if let Some(note) = note {
        store
            .attach(note, shot.id)
            .context("Screenshot saved to the library, but could not be attached")?;
    }
    if let Err(error) = media::thumbnail(paths, shot.id, &image, &file) {
        return Ok(Response::Changed(format!(
            "Screenshot saved and attachment committed; thumbnail will be retried: {error:#}"
        )));
    }
    Ok(Response::Changed("Screenshot saved; OCR queued".into()))
}

fn decode_image(store: &Store, paths: &Paths, id: i64, full: bool) -> Result<Response> {
    let shot = store
        .screenshot(id)?
        .context("Screenshot no longer exists")?;
    let image = if full {
        media::load_image(Path::new(&shot.path))?.thumbnail(2400, 2400)
    } else {
        let thumb = paths.thumbnail(id);
        if !thumb.exists() {
            media::thumbnail(
                paths,
                id,
                &media::load_image(Path::new(&shot.path))?,
                Path::new(&shot.path),
            )?;
        }
        media::load_image(&thumb)?.thumbnail(480, 320)
    }
    .into_rgba8();
    Ok(Response::Image {
        id,
        full,
        digest: shot.digest,
        updated_at: shot.updated_at,
        size: [image.width() as usize, image.height() as usize],
        rgba: image.into_raw(),
        lines: store.lines(id)?,
    })
}

fn save_config(paths: &Paths, config: &Config) -> Result<()> {
    config.validate()?;
    paths.ensure_original_location(&config.capture_directory)?;
    for folder in &config.folders {
        paths.ensure_original_location(folder)?;
    }
    let previous = Config::load(&paths.config)?;
    let changed_startup = previous
        .as_ref()
        .is_none_or(|old| old.start_at_login != config.start_at_login);
    if changed_startup {
        platform::startup(config.start_at_login, &paths.data)?;
    }
    if let Err(error) = config.save(&paths.config) {
        if changed_startup
            && let Err(rollback) = platform::startup(
                previous.as_ref().is_some_and(|old| old.start_at_login),
                &paths.data,
            )
        {
            return Err(error).context(format!(
                "Login registration rollback also failed: {rollback}"
            ));
        }
        return Err(error);
    }
    Ok(())
}

fn worker_status(paths: &Paths) -> WorkerStatus {
    let running = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(paths.data.join("reader.lock"))
        .and_then(|file| match file.try_lock() {
            Ok(()) => Ok(false),
            Err(std::fs::TryLockError::WouldBlock) => Ok(true),
            Err(std::fs::TryLockError::Error(error)) => Err(error),
        });
    let mut status = match fs::read(paths.data.join("reader.json")) {
        Ok(bytes) => match serde_json::from_slice::<WorkerStatus>(&bytes) {
            Ok(status) => status,
            Err(error) => WorkerStatus {
                state: "Reader status unavailable".into(),
                error: Some(error.to_string()),
                ..Default::default()
            },
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => WorkerStatus {
            state: "Reader not started".into(),
            ..Default::default()
        },
        Err(error) => WorkerStatus {
            state: "Reader status unavailable".into(),
            error: Some(error.to_string()),
            ..Default::default()
        },
    };
    match running {
        Ok(false) => status.state = "Reader stopped; cached search remains available".into(),
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            status.error = Some(format!("Cannot check reader liveness: {error}"))
        }
        _ => {}
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_clear_never_removes_registered_originals_and_cache_cannot_be_capture_destination()
    -> Result<()> {
        let root = tempfile::tempdir()?;
        let paths = Paths::discover(Some(root.path().to_owned()))?;
        let original = paths.thumbnail(99);
        fs::create_dir_all(original.parent().unwrap())?;
        let image = DynamicImage::ImageRgb8(image::RgbImage::new(32, 32));
        image.save(&original)?;
        let bytes = fs::read(&original)?;
        let mut store = Store::open(&paths.database())?;
        let shot = store.register_image(&original, false, None)?;
        let note = store.new_note()?;
        store.attach(note.id, shot.id)?;
        let derived = paths.thumbnail(shot.id);
        fs::write(&derived, b"derived cache")?;
        let unknown = original.parent().unwrap().join("keep.png");
        fs::write(&unknown, b"unregistered original")?;
        assert!(media::thumbnail(&paths, 99, &image, &original).is_err());
        assert!(save_capture(&mut store, &paths, image, None, &paths.cache).is_err());
        operation(Request::ClearCache, &mut store, &paths)?;
        assert_eq!(fs::read(original)?, bytes);
        assert!(unknown.exists());
        assert!(!derived.exists());
        assert_eq!(store.attachments(note.id)?[0].id, shot.id);
        Ok(())
    }
}
