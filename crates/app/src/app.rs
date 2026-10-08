use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use eframe::egui::{self, Color32, RichText};
use oxide_core::{
    Line, NoteSummary, Screenshot, WorkerStatus,
    config::{Config, Paths},
    media,
    query::Query,
};
use winit::window::Window;

use crate::{
    backend::{Backend, Request, Response},
    editor::Editor,
    lifecycle::{Action, Hotkeys, Resident, Worker},
    platform::{self, Display},
};

mod design;
mod notes;
mod screenshots;
mod settings;
mod shell;
use design::*;

pub struct Launch {
    pub background: bool,
    pub no_worker: bool,
    pub smoke: bool,
    pub onboarding: bool,
}

enum AfterSave {
    New,
    Load(i64),
    Duplicate(i64),
    Trash(i64),
    Export(i64, PathBuf),
    Quit,
}
enum Confirmation {
    Note(i64),
    Shots(Vec<i64>),
    Cache,
}
struct Texture {
    handle: egui::TextureHandle,
    digest: String,
    updated_at: i64,
    touched: Instant,
    lines: Vec<Line>,
}
struct Capture {
    image: image::DynamicImage,
    texture: egui::TextureHandle,
    start: Option<egui::Pos2>,
    note: Option<i64>,
    bounds: [u32; 4],
}

pub struct App {
    paths: Paths,
    config: Config,
    draft: Config,
    pending_config: Option<Config>,
    config_error: Option<String>,
    privacy_error: Option<String>,
    storage_error: Option<String>,
    window: Arc<Window>,
    resident: Resident,
    hotkeys: Option<Hotkeys>,
    worker: Worker,
    backend: Backend,
    editor: Editor,
    notes: Vec<NoteSummary>,
    shots: Vec<Screenshot>,
    attachments: Vec<Screenshot>,
    lines: Vec<Line>,
    linked: Vec<NoteSummary>,
    status: WorkerStatus,
    query: String,
    filter_kind: &'static str,
    filter_date: String,
    query_generation: u64,
    query_error: Option<String>,
    limit: usize,
    revision: i64,
    refresh_at: Instant,
    refresh_pending: bool,
    detail: Option<i64>,
    detail_data_id: Option<i64>,
    selected_lines: HashSet<usize>,
    selected_shots: HashSet<i64>,
    textures: HashMap<(i64, bool), Texture>,
    loading: HashSet<(i64, bool)>,
    image_errors: HashMap<(i64, bool), String>,
    after_save: Option<AfterSave>,
    pending_note_trash: Option<i64>,
    switching_note: bool,
    confirmation: Option<Confirmation>,
    settings: bool,
    settings_tab: usize,
    trash_view: bool,
    library: bool,
    all_view: bool,
    compact: bool,
    hidden: bool,
    quitting: bool,
    notice: Option<String>,
    displays: Vec<Display>,
    display_index: usize,
    capture_pending: bool,
    capture_restore: bool,
    capture_focus: bool,
    capture_region: bool,
    capture_note: Option<i64>,
    capture: Option<Capture>,
    smoke: bool,
    started: Instant,
    #[cfg(windows)]
    _tray: Option<crate::lifecycle::Tray>,
}

impl App {
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        paths: Paths,
        config: Config,
        config_error: Option<String>,
        resident: Resident,
        launch: Launch,
    ) -> Result<Self> {
        let window = creation
            .winit_window()
            .context("Native window handle is unavailable")?
            .clone();
        resident.attach_context(creation.egui_ctx.clone());
        let privacy_error = platform::set_exclusion(&window, config.recording_exclusion_enabled)
            .err()
            .map(|e| format!("{e:#}"));
        let hotkeys = Hotkeys::new(&config, resident.sender.clone(), creation.egui_ctx.clone());
        let notice = hotkeys.as_ref().err().map(|e| {
            format!("Global shortcuts unavailable: {e:#}. Launch Oxide again to summon it.")
        });
        let backend = Backend::start(paths.clone(), creation.egui_ctx.clone())?;
        let displays = platform::displays(&window);
        let mut worker = Worker::new(paths.clone());
        let mut notice = notice;
        if !launch.no_worker
            && config_error.is_none()
            && let Err(e) = worker.start()
        {
            notice = Some(format!("{e:#}"));
        }
        apply_theme(&creation.egui_ctx, &config);
        window.set_window_level(if config.always_on_top {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        });
        #[cfg(windows)]
        let tray =
            match crate::lifecycle::Tray::new(resident.sender.clone(), creation.egui_ctx.clone()) {
                Ok(tray) => Some(tray),
                Err(e) => {
                    notice = Some(format!("Tray icon: {e:#}"));
                    None
                }
            };
        let last_note = config.last_note;
        let mut app = Self {
            paths,
            draft: config.clone(),
            config,
            pending_config: None,
            config_error,
            privacy_error,
            storage_error: None,
            window,
            resident,
            hotkeys: hotkeys.ok(),
            worker,
            backend,
            editor: Editor::default(),
            notes: Vec::new(),
            shots: Vec::new(),
            attachments: Vec::new(),
            lines: Vec::new(),
            linked: Vec::new(),
            status: WorkerStatus::default(),
            query: String::new(),
            filter_kind: "date:",
            filter_date: String::new(),
            query_generation: 0,
            query_error: None,
            limit: 50,
            revision: -1,
            refresh_at: Instant::now() - Duration::from_secs(2),
            refresh_pending: false,
            detail: None,
            detail_data_id: None,
            selected_lines: HashSet::new(),
            selected_shots: HashSet::new(),
            textures: HashMap::new(),
            loading: HashSet::new(),
            image_errors: HashMap::new(),
            after_save: None,
            pending_note_trash: None,
            switching_note: false,
            confirmation: None,
            settings: launch.onboarding,
            settings_tab: 0,
            trash_view: false,
            library: false,
            all_view: false,
            compact: false,
            hidden: launch.background,
            quitting: false,
            notice,
            displays,
            display_index: 0,
            capture_pending: false,
            capture_restore: false,
            capture_focus: false,
            capture_region: false,
            capture_note: None,
            capture: None,
            smoke: launch.smoke,
            started: Instant::now(),
            #[cfg(windows)]
            _tray: tray,
        };
        if app.config_error.is_none() {
            if let Some(id) = last_note {
                app.send(Request::LoadNote(id));
            }
            app.refresh();
        }
        // The native window was constructed hidden, and its affinity was applied before display.
        if !app.hidden {
            app.window.set_visible(true);
        }
        Ok(app)
    }

    fn send(&mut self, request: Request) -> bool {
        match self.backend.send(request) {
            Ok(()) => true,
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                false
            }
        }
    }
    fn import_dropped_files(&mut self, context: &egui::Context) {
        let paths = context.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_owned())
                .collect::<Vec<_>>()
        });
        if paths.is_empty() {
            return;
        }
        let note = self
            .editor
            .note
            .as_ref()
            .filter(|note| note.deleted_at.is_none())
            .map(|note| note.id);
        let mut imported = 0;
        for path in paths {
            if media::supported(&path) && self.send(Request::Import(path, note)) {
                imported += 1;
            }
        }
        if imported > 0 {
            self.library = true;
            self.all_view = false;
            self.notice = Some(format!(
                "Importing {imported} image{}; OCR will run locally.",
                if imported == 1 { "" } else { "s" }
            ));
        }
    }
    fn refresh(&mut self) {
        if self.refresh_pending || self.config_error.is_some() || self.storage_error.is_some() {
            return;
        }
        match Query::parse(&self.query) {
            Ok(_) => {
                self.query_error = None;
            }
            Err(e) => {
                self.query_error = Some(e.to_string());
                return;
            }
        }
        self.refresh_pending = self.send(Request::Snapshot {
            generation: self.query_generation,
            query: self.query.clone(),
            trash: self.trash_view,
            note: self.editor.note.as_ref().map(|n| n.id),
            shot: self.detail,
            limit: self.limit,
        });
        self.refresh_at = Instant::now();
    }
    fn save(&mut self) {
        if let Some((generation, note)) = self.editor.begin_save()
            && !self.send(Request::SaveNote(generation, note))
        {
            self.editor
                .failed("Save could not be queued; your editing buffer was retained".into());
        }
    }
    fn defer(&mut self, action: AfterSave) {
        self.after_save = Some(action);
        if self.editor.dirty() {
            self.save();
        } else if self.editor.in_flight.is_none() {
            self.finish_action();
        }
    }
    fn finish_action(&mut self) {
        let Some(action) = self.after_save.take() else {
            return;
        };
        match action {
            AfterSave::New => {
                self.switching_note = self.send(Request::NewNote);
            }
            AfterSave::Load(id) => {
                self.switching_note = self.send(Request::LoadNote(id));
            }
            AfterSave::Duplicate(id) => {
                self.switching_note = self.send(Request::Duplicate(id));
            }
            AfterSave::Trash(id) => {
                if self.send(Request::NoteTrash(id, true)) {
                    self.pending_note_trash = Some(id);
                    self.switching_note = true;
                }
            }
            AfterSave::Export(id, path) => {
                self.send(Request::Export(id, path));
            }
            AfterSave::Quit => {
                self.window.set_visible(false);
                self.window.request_redraw();
            }
        }
    }
    fn after_save_tick(&mut self, context: &egui::Context) {
        if self.after_save.is_some() && !self.editor.dirty() && self.editor.in_flight.is_none() {
            if matches!(self.after_save, Some(AfterSave::Quit)) {
                self.after_save = None;
                self.quitting = true;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.finish_action();
            }
        }
    }

    fn process_replies(&mut self, context: &egui::Context) {
        while let Ok(response) = self.backend.receiver.try_recv() {
            match response {
                Response::Snapshot(snapshot) => {
                    self.refresh_pending = false;
                    if snapshot.generation != self.query_generation {
                        self.refresh();
                        continue;
                    }
                    if snapshot.revision != self.revision {
                        let changed: HashSet<_> = snapshot
                            .shots
                            .iter()
                            .chain(snapshot.attachments.iter())
                            .filter_map(|shot| {
                                let metadata_changed = self
                                    .shots
                                    .iter()
                                    .chain(self.attachments.iter())
                                    .find(|old| old.id == shot.id)
                                    .is_some_and(|old| {
                                        old.digest != shot.digest
                                            || old.updated_at != shot.updated_at
                                            || old.available != shot.available
                                    });
                                let stale_texture = [false, true].into_iter().any(|full| {
                                    self.textures.get(&(shot.id, full)).is_some_and(|texture| {
                                        texture.digest != shot.digest
                                            || texture.updated_at != shot.updated_at
                                    })
                                });
                                (metadata_changed || stale_texture).then_some(shot.id)
                            })
                            .collect();
                        self.textures.retain(|(id, _), _| !changed.contains(id));
                        self.image_errors.retain(|(id, _), _| !changed.contains(id));
                    }
                    self.revision = snapshot.revision;
                    self.notes = snapshot.notes;
                    self.shots = snapshot.shots;
                    self.attachments = snapshot.attachments;
                    if snapshot.detail == self.detail {
                        if self.lines != snapshot.lines {
                            self.selected_lines.clear();
                        }
                        self.lines = snapshot.lines;
                        self.linked = snapshot.linked;
                        self.detail_data_id = snapshot.detail;
                    } else {
                        self.lines.clear();
                        self.linked.clear();
                        self.selected_lines.clear();
                        self.detail_data_id = None;
                    }
                    self.status = snapshot.status;
                    if let Some(note) = snapshot.note
                        && !self.editor.dirty()
                        && self.editor.in_flight.is_none()
                    {
                        self.editor.load(note);
                    }
                }
                Response::Note(note) => {
                    self.switching_note = false;
                    self.config.last_note = Some(note.id);
                    self.draft.last_note = Some(note.id);
                    self.editor.load(note);
                    self.query_generation += 1;
                    self.detail = None;
                    self.detail_data_id = None;
                    self.lines.clear();
                    self.linked.clear();
                    self.selected_lines.clear();
                    self.refresh_pending = false;
                    self.refresh();
                    self.save_preferences(self.config.clone(), false);
                }
                Response::Saved(generation, note) => {
                    self.editor.saved(generation, &note);
                    self.refresh_at = Instant::now() - Duration::from_secs(1);
                }
                Response::SaveFailed(error) => {
                    self.editor.failed(error);
                    self.after_save = None;
                }
                Response::NoteTrashed(id) => {
                    if self.pending_note_trash == Some(id) {
                        self.pending_note_trash = None;
                        self.switching_note = false;
                        if self.editor.note.as_ref().is_some_and(|note| note.id == id) {
                            self.editor = Editor::default();
                            self.attachments.clear();
                            self.detail = None;
                            self.detail_data_id = None;
                            self.lines.clear();
                            self.linked.clear();
                            self.selected_lines.clear();
                        }
                        self.notice =
                            Some("Note moved to note trash; screenshots were retained.".into());
                        self.refresh_pending = false;
                        self.refresh();
                    }
                }
                Response::Captured(image) => {
                    self.capture_pending = false;
                    self.notice = Some(if self.capture_region {
                        "Capture ready; select a region to save it.".into()
                    } else {
                        "Screenshot captured; saving locally…".into()
                    });
                    if self.capture_region && self.capture_restore {
                        let rgba = image.to_rgba8();
                        let texture = context.load_texture(
                            "capture-selection",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [image.width() as usize, image.height() as usize],
                                rgba.as_raw(),
                            ),
                            egui::TextureOptions::LINEAR,
                        );
                        let bounds = [0, 0, image.width(), image.height()];
                        self.capture = Some(Capture {
                            image,
                            texture,
                            start: None,
                            note: self.capture_note,
                            bounds,
                        });
                    } else if !self.capture_region {
                        self.send(Request::SaveCapture(
                            image,
                            self.capture_note,
                            self.config.capture_directory.clone(),
                        ));
                    }
                    if self.capture_restore && self.privacy_error.is_none() {
                        self.hidden = false;
                        self.window.set_visible(true);
                        if self.capture_focus {
                            self.window.focus_window();
                        }
                    }
                }
                Response::CaptureFailed(error) => {
                    self.capture_pending = false;
                    if self.capture_restore && self.privacy_error.is_none() {
                        self.hidden = false;
                        self.window.set_visible(true);
                    }
                    self.notice = Some(error);
                }
                Response::Image {
                    id,
                    full,
                    digest,
                    updated_at,
                    size,
                    rgba,
                    lines,
                } => {
                    self.loading.remove(&(id, full));
                    if full && self.detail != Some(id) {
                        continue;
                    }
                    if self.hidden
                        || self
                            .shots
                            .iter()
                            .chain(self.attachments.iter())
                            .find(|shot| shot.id == id)
                            .is_some_and(|shot| {
                                shot.digest != digest || shot.updated_at != updated_at
                            })
                    {
                        continue;
                    }
                    let texture = context.load_texture(
                        format!("shot-{id}-{full}-{digest}"),
                        egui::ColorImage::from_rgba_unmultiplied(size, &rgba),
                        egui::TextureOptions::LINEAR,
                    );
                    self.textures.insert(
                        (id, full),
                        Texture {
                            handle: texture,
                            digest,
                            updated_at,
                            touched: Instant::now(),
                            lines,
                        },
                    );
                    while self.textures.len() > 64 {
                        if let Some(key) = self
                            .textures
                            .iter()
                            .filter(|((id, full), _)| !(*full && Some(*id) == self.detail))
                            .min_by_key(|(_, texture)| texture.touched)
                            .map(|(key, _)| *key)
                        {
                            self.textures.remove(&key);
                        } else {
                            break;
                        }
                    }
                }
                Response::ImageFailed(id, full, error) => {
                    self.loading.remove(&(id, full));
                    self.image_errors.insert((id, full), error);
                }
                Response::Configured(config) => {
                    self.pending_config = None;
                    self.config = config;
                    self.draft.recording_exclusion_enabled =
                        self.config.recording_exclusion_enabled;
                    self.draft.last_note = self.config.last_note;
                    apply_theme(context, &self.config);
                    self.window.set_window_level(if self.config.always_on_top {
                        winit::window::WindowLevel::AlwaysOnTop
                    } else {
                        winit::window::WindowLevel::Normal
                    });
                    if self.config.last_note != self.editor.note.as_ref().map(|note| note.id) {
                        let mut current = self.config.clone();
                        current.last_note = self.editor.note.as_ref().map(|note| note.id);
                        self.save_preferences(current, false);
                    }
                }
                Response::ConfigFailed(error) => {
                    self.pending_config = None;
                    if let Err(e) = platform::set_exclusion(
                        &self.window,
                        self.config.recording_exclusion_enabled,
                    ) {
                        self.privacy_error = Some(format!("Configuration rollback failed: {e:#}"));
                        self.window.set_visible(false);
                        self.hidden = true;
                    }
                    self.notice = Some(error);
                    self.draft.recording_exclusion_enabled =
                        self.config.recording_exclusion_enabled;
                    self.draft.last_note = self.config.last_note;
                    self.restore_shortcuts();
                }
                Response::Changed(message) => {
                    if message.starts_with("Moved ")
                        && message.contains("originals to system trash")
                    {
                        self.selected_shots.clear();
                        self.detail = None;
                        self.detail_data_id = None;
                        self.lines.clear();
                        self.linked.clear();
                        self.selected_lines.clear();
                    }
                    self.notice = Some(message);
                    self.refresh_pending = false;
                    self.refresh();
                }
                Response::Error(error) => {
                    self.switching_note = false;
                    self.pending_note_trash = None;
                    self.notice = Some(error);
                    self.refresh_pending = false;
                }
                Response::StorageFailed(error) => {
                    self.storage_error = Some(error);
                    self.refresh_pending = false;
                }
            }
        }
    }

    fn save_preferences(&mut self, config: Config, treat_window: bool) {
        if self.pending_config.is_some() || self.config_error.is_some() {
            return;
        }
        if let Err(e) = Hotkeys::validate(&config).and_then(|_| config.validate()) {
            self.notice = Some(format!("{e:#}"));
            return;
        }
        let shortcuts_changed = [
            &config.summon_shortcut,
            &config.hide_shortcut,
            &config.capture_shortcut,
            &config.display_shortcut,
        ] != [
            &self.config.summon_shortcut,
            &self.config.hide_shortcut,
            &self.config.capture_shortcut,
            &self.config.display_shortcut,
        ];
        if shortcuts_changed {
            let Some(context) = self.resident.context() else {
                self.notice = Some("Native shortcut context is unavailable".into());
                return;
            };
            if let Some(hotkeys) = &mut self.hotkeys {
                if let Err(error) = hotkeys.update(&config, self.resident.sender.clone(), context) {
                    self.notice = Some(format!("{error:#}"));
                    return;
                }
            } else {
                match Hotkeys::new(&config, self.resident.sender.clone(), context) {
                    Ok(hotkeys) => self.hotkeys = Some(hotkeys),
                    Err(error) => {
                        self.notice = Some(format!("{error:#}"));
                        return;
                    }
                }
            }
        }
        if treat_window {
            let focused = self.window.has_focus();
            self.window.set_visible(false);
            match platform::set_exclusion(&self.window, config.recording_exclusion_enabled) {
                Ok(()) => {
                    self.privacy_error = None;
                    if !self.hidden {
                        self.window.set_visible(true);
                        if focused {
                            self.window.focus_window();
                        }
                    }
                }
                Err(e) => {
                    self.privacy_error = Some(format!("{e:#}"));
                    self.hidden = true;
                    self.notice=Some("Window hidden after exclusion failure. Run oxide-app --normal to disable exclusion and recover.".into());
                    if shortcuts_changed {
                        self.restore_shortcuts();
                    }
                    return;
                }
            }
        }
        self.pending_config = Some(config.clone());
        if !self.send(Request::Config(config)) {
            self.pending_config = None;
            if let Err(e) =
                platform::set_exclusion(&self.window, self.config.recording_exclusion_enabled)
            {
                self.privacy_error = Some(e.to_string());
                self.hidden = true;
                self.window.set_visible(false);
            }
            if shortcuts_changed {
                self.restore_shortcuts();
            }
        }
    }
    fn restore_shortcuts(&mut self) {
        if let Some(hotkeys) = &mut self.hotkeys
            && let Some(context) = self.resident.context()
            && let Err(error) = hotkeys.update(&self.config, self.resident.sender.clone(), context)
        {
            self.hotkeys = None;
            self.notice = Some(format!(
                "Shortcut rollback failed; shortcuts disabled. Relaunch Oxide to summon it: {error:#}"
            ));
        }
    }
    fn toggle_exclusion(&mut self, enabled: bool) {
        let mut config = self.config.clone();
        config.recording_exclusion_enabled = enabled;
        self.save_preferences(config, true);
    }
    fn action(&mut self, action: Action, context: &egui::Context) {
        match action {
            Action::Show => {
                if self.capture_pending {
                    self.capture_restore = true;
                    self.capture_focus = true;
                    return;
                }
                if self.privacy_error.is_none() {
                    if let Err(error) = platform::set_exclusion(
                        &self.window,
                        self.pending_config
                            .as_ref()
                            .unwrap_or(&self.config)
                            .recording_exclusion_enabled,
                    ) {
                        self.privacy_error = Some(format!("{error:#}"));
                        self.hidden = true;
                        self.window.set_visible(false);
                        return;
                    }
                    self.hidden = false;
                    self.window.set_visible(true);
                    self.window.focus_window();
                    self.refresh();
                }
            }
            Action::Toggle => {
                if self.hidden {
                    self.action(Action::Show, context);
                } else {
                    self.action(Action::Hide, context);
                }
            }
            Action::Hide => {
                self.capture_restore = false;
                self.save();
                self.settings = false;
                context.send_viewport_cmd_to(
                    egui::ViewportId::from_hash_of("workspace-settings"),
                    egui::ViewportCommand::Close,
                );
                self.hidden = true;
                self.window.set_visible(false);
                self.textures.clear();
            }
            Action::Region => self.start_capture(true),
            Action::Display => self.start_capture(false),
            Action::Quit => {
                if self.editor.dirty() {
                    self.defer(AfterSave::Quit);
                } else {
                    self.quitting = true;
                    context.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Action::Normal => {
                self.toggle_exclusion(false);
                if self.privacy_error.is_none() {
                    self.action(Action::Show, context);
                }
            }
        }
    }
    fn start_capture(&mut self, region: bool) {
        if self.capture_pending
            || self.capture.is_some()
            || self.privacy_error.is_some()
            || self.config_error.is_some()
            || self.storage_error.is_some()
        {
            return;
        }
        self.displays = platform::displays(&self.window);
        let Some(display) = self.displays.get(self.display_index).cloned() else {
            self.notice = Some("No selected display is available".into());
            return;
        };
        self.save();
        self.capture_restore = !self.hidden || region;
        self.capture_focus = self.window.has_focus() || region;
        self.capture_region = region;
        self.capture_note = self
            .editor
            .note
            .as_ref()
            .filter(|note| note.deleted_at.is_none())
            .map(|note| note.id);
        if let Err(e) = platform::hide_for_capture(&self.window) {
            self.window.set_visible(true);
            self.notice = Some(format!("{e:#}"));
            return;
        }
        self.hidden = true;
        self.capture_pending = self.send(Request::Capture(display));
        if !self.capture_pending {
            self.hidden = false;
            self.window.set_visible(true);
        }
    }
    fn picture(&mut self, id: i64, full: bool) -> Option<(egui::TextureId, egui::Vec2)> {
        if let Some(texture) = self.textures.get_mut(&(id, full)) {
            texture.touched = Instant::now();
            return Some((texture.handle.id(), texture.handle.size_vec2()));
        }
        if !self.loading.contains(&(id, full))
            && !self.image_errors.contains_key(&(id, full))
            && self.send(Request::Image(id, full))
        {
            self.loading.insert((id, full));
        }
        None
    }
    fn select_shot(&mut self, id: i64) {
        self.textures.retain(|(_, full), _| !*full);
        self.detail = Some(id);
        self.detail_data_id = None;
        self.lines.clear();
        self.linked.clear();
        self.query_generation += 1;
        self.selected_lines.clear();
        self.refresh_pending = false;
        self.refresh();
    }

    fn export_current(&mut self) {
        if let Some(id) = self.editor.note.as_ref().map(|note| note.id)
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Choose export directory")
                .pick_folder()
        {
            self.defer(AfterSave::Export(id, path));
        }
    }

    fn confirm(&mut self, context: &egui::Context) {
        let Some(confirmation) = &self.confirmation else {
            return;
        };
        let description=match confirmation{Confirmation::Note(_)=>"Move this note to note trash? Its images stay in the library.".into(),Confirmation::Shots(ids)=>format!("Move exactly {} selected original image(s) to recoverable system trash?",ids.len()),Confirmation::Cache=>"Clear OCR/thumbnail caches? Authored notes, attachment relationships, and originals will be retained.".into()};
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Confirm action")
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.label(description);
                ui.horizontal(|ui| {
                    accept = ui.button("Confirm").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if accept {
            if let Some(confirmation) = self.confirmation.take() {
                match confirmation {
                    Confirmation::Note(id) => self.defer(AfterSave::Trash(id)),
                    Confirmation::Shots(ids) => {
                        self.send(Request::TrashShots(ids));
                    }
                    Confirmation::Cache => {
                        self.send(Request::ClearCache);
                        self.textures.clear();
                        self.image_errors.clear();
                    }
                }
            }
        } else if cancel {
            self.confirmation = None;
        }
    }

    fn capture_selection(&mut self, ui: &mut egui::Ui) {
        ui.heading("Select a region");
        ui.label("Drag a rectangle on the captured display. Release to save. Escape cancels.");
        let mut crop = None;
        let mut cancel = false;
        if ui.button("Cancel").clicked() {
            cancel = true;
        }
        if let Some(capture) = &mut self.capture {
            ui.horizontal_wrapped(|ui| {
                let [x, y, width, height] = &mut capture.bounds;
                ui.label("X");
                ui.add(egui::DragValue::new(x).range(0..=capture.image.width().saturating_sub(1)));
                ui.label("Y");
                ui.add(egui::DragValue::new(y).range(0..=capture.image.height().saturating_sub(1)));
                ui.label("Width");
                ui.add(egui::DragValue::new(width).range(1..=capture.image.width() - *x));
                ui.label("Height");
                ui.add(egui::DragValue::new(height).range(1..=capture.image.height() - *y));
                if ui.button("Save region").clicked() {
                    crop = Some((
                        *x,
                        *y,
                        (*width).min(capture.image.width() - *x),
                        (*height).min(capture.image.height() - *y),
                    ));
                }
            });
            let response = ui.add(
                egui::Image::new((capture.texture.id(), capture.texture.size_vec2()))
                    .max_size(ui.available_size())
                    .sense(egui::Sense::drag()),
            );
            if response.drag_started() {
                capture.start = response.interact_pointer_pos();
            }
            if let (Some(start), Some(end)) = (capture.start, response.interact_pointer_pos()) {
                let rect = egui::Rect::from_two_pos(start, end).intersect(response.rect);
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(2.0, Color32::from_rgb(190, 110, 76)),
                    egui::StrokeKind::Inside,
                );
                if response.drag_stopped() && rect.width() > 2.0 && rect.height() > 2.0 {
                    let x = ((rect.left() - response.rect.left()) / response.rect.width()
                        * capture.image.width() as f32)
                        .floor()
                        .max(0.0) as u32;
                    let y = ((rect.top() - response.rect.top()) / response.rect.height()
                        * capture.image.height() as f32)
                        .floor()
                        .max(0.0) as u32;
                    let width = (rect.width() / response.rect.width()
                        * capture.image.width() as f32)
                        .round() as u32;
                    let height = (rect.height() / response.rect.height()
                        * capture.image.height() as f32)
                        .round() as u32;
                    crop = Some((
                        x,
                        y,
                        width.min(capture.image.width() - x),
                        height.min(capture.image.height() - y),
                    ));
                }
            }
        }
        if let Some((x, y, width, height)) = crop
            && let Some(capture) = self.capture.take()
        {
            let image = capture.image.crop_imm(x, y, width, height);
            self.notice = Some("Saving selected region locally…".into());
            self.send(Request::SaveCapture(
                image,
                capture.note,
                self.config.capture_directory.clone(),
            ));
        } else if cancel {
            self.capture = None;
        }
    }

    fn recovery(&mut self, ui: &mut egui::Ui) {
        ui.heading("Oxide needs attention");
        if let Some(error) = &self.config_error {
            ui.label(error);
            ui.label(self.paths.config.display().to_string());
            if ui.button("Reload repaired configuration").clicked() {
                match Config::load(&self.paths.config){Ok(Some(config))=>{self.config_error=None;self.config=config.clone();self.draft=config;self.privacy_error=platform::set_exclusion(&self.window,self.config.recording_exclusion_enabled).err().map(|e|e.to_string());self.refresh();},Ok(None)=>self.notice=Some("Configuration is missing; restore it or restart to configure a new workspace".into()),Err(e)=>self.notice=Some(e.to_string())}
            }
        }
        if let Some(error) = &self.storage_error {
            ui.label(error);
            if ui.button("Retry database").clicked() {
                match Backend::start(self.paths.clone(), ui.ctx().clone()) {
                    Ok(backend) => {
                        self.backend = backend;
                        self.storage_error = None;
                        self.refresh();
                    }
                    Err(e) => self.notice = Some(e.to_string()),
                }
            }
        }
        if let Some(error) = &self.privacy_error {
            ui.label(error);
            if ui
                .button("Use Oxide with recording exclusion Off")
                .clicked()
            {
                self.toggle_exclusion(false);
            }
        }
        if ui.button("Open data directory").clicked()
            && let Err(e) = open::that(&self.paths.data)
        {
            self.notice = Some(e.to_string());
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, context: &egui::Context, _: &mut eframe::Frame) {
        self.import_dropped_files(context);
        self.process_replies(context);
        while let Ok(action) = self.resident.receiver.try_recv() {
            self.action(action, context);
        }
        // Reapply before content is rendered after resume/native-handle recreation.
        if !self.hidden
            && self.privacy_error.is_none()
            && platform::exclusion_supported()
            && let Err(error) = platform::set_exclusion(
                &self.window,
                self.pending_config
                    .as_ref()
                    .unwrap_or(&self.config)
                    .recording_exclusion_enabled,
            )
        {
            self.privacy_error = Some(format!(
                "Recording-exclusion treatment failed: {error:#}. Relaunch with --normal to recover."
            ));
            self.hidden = true;
            self.window.set_visible(false);
        }
        if self.editor.dirty()
            && self.editor.in_flight.is_none()
            && self.editor.error.is_none()
            && self
                .editor
                .changed_at
                .is_some_and(|time| time.elapsed() >= Duration::from_millis(500))
        {
            self.save();
        }
        self.after_save_tick(context);
        if !self.hidden && self.refresh_at.elapsed() >= Duration::from_secs(1) {
            self.refresh();
        }
        if self.smoke && self.started.elapsed() > Duration::from_secs(3) {
            context.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        context.request_repaint_after(Duration::from_millis(250));
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        if context.input(|input| input.viewport().close_requested())
            && !self.smoke
            && !self.quitting
        {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.action(Action::Hide, &context);
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::N)) {
            self.new_note();
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save();
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("search")));
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            context.memory_mut(|memory| memory.request_focus(egui::Id::new("search")));
        }
        if context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.capture.is_some() {
                self.capture = None;
            } else if self.confirmation.is_some() {
                self.confirmation = None;
            } else if self.detail.is_some() {
                self.detail = None;
            } else if self.settings {
                self.settings = false;
                context.send_viewport_cmd_to(
                    egui::ViewportId::from_hash_of("workspace-settings"),
                    egui::ViewportCommand::Close,
                );
            } else {
                self.action(Action::Hide, &context);
            }
        }
        let p = Palette::of(ui);
        egui::Panel::top("header")
            .frame(panel_frame(p.rail, 12))
            .show(ui, |ui| self.header(ui));
        egui::Panel::bottom("status")
            .frame(panel_frame(p.rail, 8))
            .show(ui, |ui| self.status_bar(ui));
        if self.config_error.is_some()
            || self.storage_error.is_some()
            || self.privacy_error.is_some()
        {
            egui::CentralPanel::default().show(ui, |ui| self.recovery(ui));
            return;
        }
        if self.capture.is_some() {
            egui::CentralPanel::default().show(ui, |ui| self.capture_selection(ui));
            return;
        }
        if !self.compact {
            egui::Panel::left("workspace-rail")
                .resizable(false)
                .exact_size(212.0)
                .frame(panel_frame(p.rail, 16))
                .show(ui, |ui| self.workspace_rail(ui));
        }
        if !self.library && !self.compact {
            egui::Panel::left("notes-sidebar")
                .resizable(true)
                .default_size(246.0)
                .frame(panel_frame(p.canvas, 16))
                .show(ui, |ui| self.note_sidebar(ui));
        }
        egui::CentralPanel::default()
            .frame(panel_frame(p.canvas, 24))
            .show(ui, |ui| {
                if self.library {
                    self.library(ui);
                } else {
                    self.note_editor(ui);
                }
            });
        self.detail(&context);
        self.settings(&context);
        self.confirm(&context);
    }
}
