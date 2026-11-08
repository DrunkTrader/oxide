use super::*;

impl App {
    pub(super) fn navigate(&mut self, library: bool, all: bool, trash: bool) {
        self.library = library;
        self.all_view = all;
        self.trash_view = trash;
        self.query_generation += 1;
        self.refresh_pending = false;
        self.refresh();
    }

    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let brand = self
                .brand
                .get_or_insert_with(|| brand_texture(ui.ctx()))
                .clone();
            let mark = egui::Image::new(&brand)
                .fit_to_exact_size(egui::vec2(24.0, 24.0))
                .corner_radius(7);
            let button = egui::Button::new((mark, bold(RichText::new("Oxide").size(17.0))))
                .frame_when_inactive(false)
                .corner_radius(10)
                .min_size(egui::vec2(0.0, 36.0));
            egui::containers::menu::MenuButton::from_button(button).ui(ui, |ui| {
                caption(ui, "A local workspace");
                if ui
                    .button(if self.compact {
                        "Expand workspace"
                    } else {
                        "Compact note panel"
                    })
                    .clicked()
                {
                    self.compact = !self.compact;
                    let size = if self.compact {
                        winit::dpi::LogicalSize::new(480.0, 620.0)
                    } else {
                        winit::dpi::LogicalSize::new(1120.0, 760.0)
                    };
                    let _ = self.window.request_inner_size(size);
                    ui.close();
                }
                if ui.button("Hide workspace").clicked() {
                    self.action(Action::Hide, &ui.ctx().clone());
                    ui.close();
                }
                if ui.button("Quit Oxide").clicked() {
                    self.action(Action::Quit, &ui.ctx().clone());
                    ui.close();
                }
            });
            // Right cluster is laid out from the window edge inward, so Settings stays flush right.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                if ui
                    .add(
                        egui::Button::new("Settings")
                            .corner_radius(10)
                            .min_size(egui::vec2(0.0, 34.0)),
                    )
                    .clicked()
                {
                    self.draft = self.config.clone();
                    self.settings = true;
                }
                if !self.compact {
                    let themes = ["system", "light", "dark"];
                    let mut i = themes
                        .iter()
                        .position(|t| *t == self.config.theme)
                        .unwrap_or(0);
                    if segmented(ui, "theme-quick", &["Auto", "Light", "Dark"], &mut i) {
                        let mut config = self.config.clone();
                        config.theme = themes[i].into();
                        self.save_preferences(config, false);
                    }
                    self.privacy_control(ui);
                }
                if quiet(ui, "+")
                    .on_hover_text("New note · Ctrl/Cmd+N")
                    .clicked()
                {
                    self.new_note();
                }
                // Search takes up to 380 px, centred in the space left between the brand and "+".
                let width = (ui.available_width() - 16.0).clamp(100.0, 380.0);
                ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
                let hint = if cfg!(target_os = "macos") {
                    "⌘K"
                } else {
                    "Ctrl K"
                };
                if search_field(
                    ui,
                    &mut self.query,
                    width,
                    &format!("Search notes and screenshots    {hint}"),
                )
                .changed()
                {
                    self.changed_query();
                }
            });
        });
        if let Some(error) = &self.query_error {
            ui.colored_label(p.danger, error);
        }
    }

    pub(super) fn privacy_control(&mut self, ui: &mut egui::Ui) {
        let mut enabled = self
            .pending_config
            .as_ref()
            .unwrap_or(&self.config)
            .recording_exclusion_enabled;
        let label = if self.privacy_error.is_some() {
            "Exclusion failed"
        } else if !platform::exclusion_supported() {
            "Exclusion unavailable"
        } else if enabled {
            "Exclusion on"
        } else {
            "Exclusion off"
        };
        let response = ui.add_enabled(
            platform::exclusion_supported() && self.pending_config.is_none(),
            egui::Checkbox::new(&mut enabled, RichText::new(label).size(11.0)),
        );
        if response.on_hover_text("Recording exclusion applies to validated capture paths. Recorder compatibility is not automatically verified.").changed() { self.toggle_exclusion(enabled); }
    }

    pub(super) fn workspace_rail(&mut self, ui: &mut egui::Ui) {
        // Actions stay pinned; the lists above them scroll when they outgrow the window.
        egui::Panel::bottom("rail-actions")
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                ui.add_space(12.0);
                if primary(ui, "+ New note").clicked() {
                    self.new_note();
                }
                ui.add_space(4.0);
                ui.menu_button("Capture…", |ui| {
                    self.capture_controls(ui);
                });
                ui.add_space(12.0);
                caption(ui, "Stored on this device");
            });
        egui::ScrollArea::vertical()
            .id_salt("rail-lists")
            .show(ui, |ui| self.rail_lists(ui));
    }

    fn rail_lists(&mut self, ui: &mut egui::Ui) {
        ui.add_space(14.0);
        caption(ui, "Workspace");
        ui.add_space(4.0);
        for (label, library, all) in [
            ("All", true, true),
            ("Notes", false, false),
            ("Screenshots", true, false),
        ] {
            if nav_item(
                ui,
                label,
                !self.trash_view && self.library == library && self.all_view == all,
            ) {
                self.navigate(library, all, false);
            }
        }
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let on_today = !self.library
            && self
                .editor
                .note
                .as_ref()
                .is_some_and(|note| note.title == today);
        if nav_item(ui, "Today", on_today) {
            self.navigate(false, false, false);
            self.defer(AfterSave::Daily);
        }
        nav_indicator(ui, "workspace");
        if !self.all_tags.is_empty() {
            ui.add_space(22.0);
            caption(ui, "Tags");
            for tag in self.all_tags.clone() {
                let filter = format!("tag:{tag}");
                if nav_item(ui, &format!("#{tag}"), self.query == filter) {
                    self.query = filter;
                    self.navigate(true, true, false);
                }
            }
            nav_indicator(ui, "tags");
        }
        ui.add_space(22.0);
        caption(ui, "Collections");
        if self.config.folders.is_empty() {
            caption(ui, "Add a screenshot folder to organize your library.");
            if quiet(ui, "+ Add folder").clicked() {
                self.draft = self.config.clone();
                self.settings = true;
                self.settings_tab = 1;
            }
        } else {
            let folders = self.config.folders.clone();
            for folder in folders {
                let label = folder.file_name().unwrap_or_default().to_string_lossy();
                if nav_item(ui, &label, false) {
                    match folder_filter(&folder) {
                        Ok(filter) => {
                            self.query = filter;
                            self.navigate(true, false, false);
                        }
                        Err(error) => self.notice = Some(format!("{error:#}")),
                    }
                }
            }
        }
        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);
        caption(ui, "Recent");
        let recent = self
            .notes
            .iter()
            .take(4)
            .map(|note| (note.id, note.title.clone()))
            .collect::<Vec<_>>();
        for (id, title) in recent {
            let selected =
                !self.library && self.editor.note.as_ref().is_some_and(|note| note.id == id);
            let title = if title.is_empty() {
                "Untitled note"
            } else {
                &title
            };
            if ui.push_id(id, |ui| nav_item(ui, title, selected)).inner {
                self.navigate(false, false, false);
                self.defer(AfterSave::Load(id));
            }
        }
        if nav_item(ui, "Note trash", self.trash_view) {
            self.navigate(false, false, true);
        }
        nav_indicator(ui, "recent");
    }

    pub(super) fn new_note(&mut self) {
        self.library = false;
        self.all_view = false;
        self.trash_view = false;
        self.defer(AfterSave::New);
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = Palette::of(ui);
        ui.horizontal_wrapped(|ui| {
            let (dot, busy) = if self.editor.error.is_some() {
                (p.danger, false)
            } else if self.editor.in_flight.is_some() || self.editor.dirty() {
                (p.accent, true)
            } else {
                (p.success, false)
            };
            status_dot(ui, dot, busy);
            caption(
                ui,
                if self.editor.in_flight.is_some() {
                    "Saving…"
                } else if self.editor.error.is_some() {
                    "Save failed · buffer retained"
                } else if self.editor.dirty() {
                    "Unsaved changes"
                } else {
                    "Saved locally"
                },
            );
            if self.editor.error.is_some() && quiet(ui, "Retry save").clicked() {
                self.editor.error = None;
                self.save();
            }
            if self.editor.error.is_some()
                && quiet(ui, "Export buffer").clicked()
                && let Some(note) = self.editor.note.clone()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("oxide-recovery.md")
                    .save_file()
            {
                self.send(Request::ExportBuffer(note, path));
            }
            ui.separator();
            caption(
                ui,
                if self.status.state.is_empty() {
                    "Starting reader…"
                } else {
                    &self.status.state
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                caption(ui, "Local only");
                status_dot(ui, p.success, false);
            });
        });
        if let Some(error) = &self.editor.error {
            ui.colored_label(p.danger, error);
        }
        if let Some(error) = &self.status.error {
            caption(ui, error);
        }
        // Notices are drawn as a floating toast (see `App::ui`).
    }
}

/// The app icon as a small texture for the header brand mark.
fn brand_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let image = match image::load_from_memory(include_bytes!("../../../../assets/icon.png")) {
        Ok(image) => image
            .resize(96, 96, image::imageops::FilterType::Lanczos3)
            .into_rgba8(),
        // The PNG is embedded at build time; a decode failure shows a plain tile instead.
        Err(_) => image::RgbaImage::from_pixel(1, 1, image::Rgba([139, 124, 255, 255])),
    };
    let size = [image.width() as usize, image.height() as usize];
    ctx.load_texture(
        "brand-mark",
        egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw()),
        egui::TextureOptions::LINEAR,
    )
}
