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
            ui.menu_button(
                RichText::new("OXIDE").size(17.0).strong().color(p.text),
                |ui| {
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
                },
            );
            let reserve = if self.compact { 120.0 } else { 300.0 };
            ui.add_space((ui.available_width() - reserve - 380.0).max(8.0));
            let width = (ui.available_width() - reserve).clamp(100.0, 380.0);
            let hint = if cfg!(target_os = "macos") {
                "⌘K"
            } else {
                "Ctrl K"
            };
            let search = egui::TextEdit::singleline(&mut self.query)
                .id(egui::Id::new("search"))
                .hint_text(format!("Search your workspace…    {hint}"))
                .desired_width(width)
                .margin(egui::vec2(12.0, 7.0));
            if ui.add(search).changed() {
                self.changed_query();
            }
            if quiet(ui, "New note")
                .on_hover_text("New note · Ctrl/Cmd+N")
                .clicked()
            {
                self.new_note();
            }
            if !self.compact {
                self.privacy_control(ui);
            }
            if quiet(ui, "Settings").clicked() {
                self.draft = self.config.clone();
                self.settings = true;
            }
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
        ui.add_space(22.0);
        caption(ui, "Screenshot sources");
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
                if nav_item(ui, &format!("▸  {label}"), false) {
                    self.query = format!("in:\"{}\"", folder.display());
                    self.navigate(true, false, false);
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
            if nav_item(
                ui,
                &title,
                !self.library && self.editor.note.as_ref().is_some_and(|note| note.id == id),
            ) {
                self.navigate(false, false, false);
                self.defer(AfterSave::Load(id));
            }
        }
        if nav_item(ui, "Note trash", self.trash_view) {
            self.navigate(false, false, true);
        }
        ui.add_space((ui.available_height() - 152.0).max(20.0));
        if primary(ui, "+ New note").clicked() {
            self.new_note();
        }
        ui.add_space(4.0);
        ui.menu_button("Capture…", |ui| {
            self.capture_controls(ui);
        });
        ui.add_space(12.0);
        caption(ui, "Stored on this device");
    }

    pub(super) fn new_note(&mut self) {
        self.library = false;
        self.all_view = false;
        self.trash_view = false;
        self.defer(AfterSave::New);
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            caption(ui, self.editor.state().label());
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
                format!(
                    "{} · {} pending",
                    self.status.state.label(),
                    self.status.backlog
                ),
            );
            if quiet(ui, "Rescan").clicked() {
                self.send(Request::Rescan);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                caption(ui, "●  Local only");
            });
        });
        if let Some(error) = &self.editor.error {
            ui.colored_label(Palette::of(ui).danger, error);
        }
        if let Some(error) = &self.status.error {
            caption(ui, error);
        }
        if let Some(message) = self.notice.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(message).size(12.0));
                if quiet(ui, "Dismiss").clicked() {
                    self.notice = None;
                }
            });
        }
    }
}

fn nav_item(ui: &mut egui::Ui, text: &str, selected: bool) -> bool {
    let p = Palette::of(ui);
    let response = ui.add_sized(
        [ui.available_width(), 34.0],
        egui::Button::new(RichText::new(text).size(13.0).color(if selected {
            p.accent_text
        } else {
            p.text
        }))
        .selected(selected)
        .right_text("")
        .truncate()
        .fill(if selected {
            p.accent_soft
        } else {
            Color32::TRANSPARENT
        })
        .stroke(egui::Stroke::NONE),
    );
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                response.rect.min + egui::vec2(0.0, 8.0),
                egui::vec2(2.0, 18.0),
            ),
            1.0,
            ACCENT,
        );
    }
    response.clicked()
}
