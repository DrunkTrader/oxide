use super::*;

impl App {
    pub(super) fn filters(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Filters", |ui| {
            ui.set_max_width(300.0);
            caption(ui, "Local calendar day");
            egui::ComboBox::from_id_salt("filter-kind").selected_text(self.filter_kind).show_ui(ui, |ui| {
                for kind in ["date:", "after:", "before:"] { ui.selectable_value(&mut self.filter_kind, kind, kind); }
            });
            ui.add(egui::TextEdit::singleline(&mut self.filter_date).hint_text("YYYY-MM-DD"));
            if ui.button("Apply date").clicked() {
                if self.filter_date.len() == 10 && chrono::NaiveDate::parse_from_str(&self.filter_date, "%Y-%m-%d").is_ok() {
                    self.query.push_str(&format!(" {}{}", self.filter_kind, self.filter_date));
                    self.changed_query(); ui.close();
                } else { self.notice = Some("Use a complete calendar date in YYYY-MM-DD format".into()); }
            }
            if ui.button("Choose screenshot folder…").clicked() && let Some(folder) = rfd::FileDialog::new().pick_folder() {
                let folder = folder.to_string_lossy();
                if folder.contains('"') { self.notice = Some("The quoted folder-filter syntax cannot represent this folder name".into()); }
                else { self.query.push_str(&format!(" in:\"{folder}\"")); self.library = true; self.all_view = false; self.changed_query(); ui.close(); }
            }
            ui.separator();
            caption(ui, "Folders filter screenshots only. Dates use last edit or image capture/modification time.");
            if quiet(ui, "Clear filters").clicked() && let Ok(query) = Query::parse(&self.query) {
                self.query = query.terms.join(" "); self.changed_query(); ui.close();
            }
        });
    }

    pub(super) fn changed_query(&mut self) {
        self.limit = 50;
        self.query_generation += 1;
        self.refresh_pending = false;
        self.refresh();
    }

    pub(super) fn settings(&mut self, context: &egui::Context) {
        if !self.settings {
            return;
        }
        let mut open = true;
        egui::Window::new("Settings")
            .id(egui::Id::new("workspace-settings"))
            .open(&mut open)
            .collapsible(false)
            .default_size([660.0, 570.0])
            .show(context, |ui| {
                caption(ui, "Your workspace, on your device.");
                ui.add_space(14.0);
                ui.horizontal_wrapped(|ui| {
                    for (index, label) in ["General", "Storage", "Shortcuts", "OCR & maintenance"]
                        .iter()
                        .enumerate()
                    {
                        ui.selectable_value(&mut self.settings_tab, index, *label);
                    }
                });
                ui.add_space(14.0);
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("settings-content")
                    .max_height(390.0)
                    .show(ui, |ui| {
                        ui.set_min_height(330.0);
                        ui.add_space(12.0);
                        match self.settings_tab {
                            0 => self.general_settings(ui),
                            1 => self.storage_settings(ui),
                            2 => self.shortcut_settings(ui),
                            _ => self.ocr_settings(ui),
                        }
                    });
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    caption(ui, "Changes apply when saved.");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(self.pending_config.is_none(), |ui| {
                            if primary(ui, "Save settings").clicked() {
                                self.save_preferences(self.draft.clone(), false);
                            }
                        });
                    });
                });
            });
        if !open {
            self.settings = false;
        }
    }

    fn general_settings(&mut self, ui: &mut egui::Ui) {
        section(
            ui,
            "Appearance",
            "A quiet workspace that follows your system.",
        );
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label("Theme");
            egui::ComboBox::from_id_salt("theme")
                .selected_text(&self.draft.theme)
                .show_ui(ui, |ui| {
                    for theme in ["system", "light", "dark"] {
                        ui.selectable_value(&mut self.draft.theme, theme.into(), theme);
                    }
                });
        });
        ui.checkbox(&mut self.draft.always_on_top, "Keep Oxide on top");
        ui.checkbox(&mut self.draft.start_at_login, "Launch at login");
        ui.checkbox(&mut self.draft.reduce_motion, "Reduce motion");
        caption(
            ui,
            "Removes nonessential interface transitions while keeping status changes immediate.",
        );
        ui.add_space(24.0);
        section(
            ui,
            "Recording exclusion",
            "Control whether supported screen capture paths include Oxide.",
        );
        ui.add_space(12.0);
        self.privacy_control(ui);
        caption(
            ui,
            if platform::exclusion_supported() {
                "The toggle applies immediately. Recorder compatibility needs validation for your capture configuration."
            } else {
                "This platform has no supported recording-exclusion mechanism. Notes and capture remain available."
            },
        );
    }

    fn storage_settings(&mut self, ui: &mut egui::Ui) {
        section(
            ui,
            "Storage",
            "Originals stay in the folders you choose. Nothing is uploaded.",
        );
        ui.add_space(16.0);
        ui.label(RichText::new("Capture destination").strong());
        caption(ui, self.draft.capture_directory.display().to_string());
        if ui.button("Choose destination…").clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
        {
            self.draft.capture_directory = path;
        }
        ui.add_space(24.0);
        ui.label(RichText::new("Screenshot folders").strong());
        caption(ui, "Watched folders appear as collections in your sidebar.");
        let mut remove = None;
        for (index, folder) in self.draft.folders.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(folder.display().to_string()).size(12.0))
                        .truncate(),
                );
                if quiet(ui, "Remove").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            self.draft.folders.remove(index);
        }
        if ui.button("Add folder…").clicked()
            && let Some(path) = rfd::FileDialog::new().pick_folder()
            && !self.draft.folders.contains(&path)
        {
            self.draft.folders.push(path);
        }
        ui.add_space(20.0);
        if quiet(ui, "Open data directory").clicked()
            && let Err(e) = open::that(&self.paths.data)
        {
            self.notice = Some(e.to_string());
        }
    }

    fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        section(
            ui,
            "Keyboard shortcuts",
            "Global shortcuts work even when another app is focused.",
        );
        ui.add_space(16.0);
        egui::Grid::new("shortcuts")
            .spacing([24.0, 12.0])
            .show(ui, |ui| {
                for (label, value) in [
                    ("Summon / hide", &mut self.draft.summon_shortcut),
                    ("Hide all", &mut self.draft.hide_shortcut),
                    ("Capture region", &mut self.draft.capture_shortcut),
                    ("Capture display", &mut self.draft.display_shortcut),
                ] {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(value).font(egui::TextStyle::Monospace));
                    ui.end_row();
                }
            });
        ui.add_space(24.0);
        caption(ui, "In-app shortcuts");
        ui.label("Ctrl/Cmd + K or F   Search\nCtrl/Cmd + N   New note\nCtrl/Cmd + S   Save\nEscape   Close panel / hide");
    }

    fn ocr_settings(&mut self, ui: &mut egui::Ui) {
        section(
            ui,
            "Local OCR",
            "Text recognition runs in a separate process on this device.",
        );
        ui.add_space(12.0);
        ui.checkbox(&mut self.draft.worker_paused, "Pause background OCR");
        ui.horizontal(|ui| {
            ui.label("CPU threads");
            ui.add(egui::DragValue::new(&mut self.draft.threads).range(1..=16));
        });
        caption(ui, format!("Models: {}", self.paths.models().display()));
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Restart reader").clicked()
                && let Err(e) = self.worker.restart()
            {
                self.notice = Some(format!("{e:#}"));
            }
            if ui.button("Rescan sources").clicked() {
                self.send(Request::Rescan);
            }
            if ui.button("Reindex library").clicked() {
                self.send(Request::Reindex(None));
            }
        });
        ui.add_space(10.0);
        ui.label(RichText::new("Reader status").strong());
        caption(
            ui,
            format!(
                "{} · {} pending",
                self.status.state.label(),
                self.status.backlog
            ),
        );
        for source in &self.status.sources {
            caption(
                ui,
                format!(
                    "{} · {}",
                    source.path.display(),
                    if !source.available {
                        "Unavailable"
                    } else if source.watched {
                        "Watching"
                    } else {
                        "Watch unavailable"
                    }
                ),
            );
            if let Some(error) = &source.error {
                ui.colored_label(Palette::of(ui).danger, error);
            }
        }
        if let Some(time) = self
            .status
            .reconciled_at
            .and_then(chrono::DateTime::from_timestamp_millis)
        {
            caption(
                ui,
                format!(
                    "Last reconciliation: {}",
                    time.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                ),
            );
        }
        if let Some(error) = &self.status.error {
            ui.colored_label(Palette::of(ui).danger, error);
        }
        ui.add_space(24.0);
        section(
            ui,
            "Maintenance",
            "Authored notes and original images are kept separate from caches.",
        );
        ui.add_space(12.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("Back up database…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("oxide-backup.db")
                    .save_file()
            {
                self.send(Request::Backup(path));
            }
            if quiet(ui, "Clear derived cache…").clicked() {
                self.confirmation = Some(Confirmation::Cache);
            }
        });
        caption(
            ui,
            "Database backups exclude image originals. Keep the capture folder with your backup.",
        );
    }
}
