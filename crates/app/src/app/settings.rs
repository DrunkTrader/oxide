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
            if ui.button("Choose screenshot folder…").clicked()
                && let Some(folder) = rfd::FileDialog::new().pick_folder()
            {
                match folder_filter(&folder) {
                    Ok(filter) => {
                        self.query.push(' ');
                        self.query.push_str(&filter);
                        self.library = true;
                        self.all_view = false;
                        self.changed_query();
                        ui.close();
                    }
                    Err(error) => self.notice = Some(format!("{error:#}")),
                }
            }
            ui.separator();
            caption(ui, "Folders filter screenshots only. Dates use last edit or image capture/modification time.");
            if quiet(ui, "Clear filters").clicked() {
                self.query = terms_without_filters(&self.query).join(" ");
                self.changed_query();
                ui.close();
            }
        });
    }

    pub(super) fn changed_query(&mut self) {
        self.limit = 50;
        self.query_generation += 1;
        self.refresh_pending = false;
        self.refresh();
    }

    /// Settings as a sheet over the window (dimmed backdrop). It stays inside Oxide's own window
    /// on purpose: recording exclusion protects that window, so a separate one could leak into a share.
    pub(super) fn settings(&mut self, context: &egui::Context) {
        if !self.settings {
            return;
        }
        let dark = context.global_style().visuals.dark_mode;
        let p = Palette::new(dark);
        let screen = context.content_rect();
        let w = (screen.width() - 80.0).clamp(560.0, 760.0);
        let h = (screen.height() - 120.0).clamp(420.0, 680.0);
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("workspace-settings"))
            .backdrop_color(Color32::from_black_alpha(if dark { 150 } else { 70 }))
            .frame(
                egui::Frame::new()
                    .fill(p.raised)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(18)
                    .inner_margin(egui::Margin::same(24))
                    .shadow(context.global_style().visuals.window_shadow),
            )
            .show(context, |ui| {
                entrance(ui, "settings-sheet", 0);
                ui.set_width(w);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Settings").size(22.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if quiet(ui, "Close").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(12.0);
                segmented(
                    ui,
                    "settings-tabs",
                    &["General", "Privacy", "Storage", "Shortcuts", "OCR"],
                    &mut self.settings_tab,
                );
                ui.add_space(16.0);
                egui::ScrollArea::vertical()
                    .id_salt(("settings-content", self.settings_tab))
                    .max_height(h - 170.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width() - 12.0);
                        entrance(ui, "settings-tab", self.settings_tab);
                        match self.settings_tab {
                            0 => self.general_settings(ui),
                            1 => self.privacy_settings(ui),
                            2 => self.storage_settings(ui),
                            3 => self.shortcut_settings(ui),
                            _ => self.ocr_settings(ui),
                        }
                        ui.add_space(8.0);
                    });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    caption(ui, "Changes apply when saved.");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_enabled_ui(self.pending_config.is_none(), |ui| {
                            if primary(ui, "Save").clicked() {
                                self.save_preferences(self.draft.clone(), false);
                                close = true;
                            }
                        });
                        if quiet(ui, "Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close || modal.backdrop_response.clicked() {
            self.settings = false;
        }
    }

    fn general_settings(&mut self, ui: &mut egui::Ui) {
        group_title(ui, "Appearance", "Light, dark, or follow your system.");
        form_group(ui, |ui| {
            form_row(ui, "Theme", "", |ui| {
                let themes = ["system", "light", "dark"];
                let mut i = themes
                    .iter()
                    .position(|t| *t == self.draft.theme)
                    .unwrap_or(0);
                if segmented(ui, "theme-draft", &["Auto", "Light", "Dark"], &mut i) {
                    self.draft.theme = themes[i].into();
                }
            });
        });
        ui.add_space(18.0);
        group_title(ui, "Window", "");
        form_group(ui, |ui| {
            form_row(ui, "Keep Oxide on top", "Stays above other windows", |ui| {
                ui.checkbox(&mut self.draft.always_on_top, "");
            });
            ui.separator();
            form_row(
                ui,
                "Launch at login",
                "Starts hidden in the background",
                |ui| {
                    ui.checkbox(&mut self.draft.start_at_login, "");
                },
            );
        });
    }

    fn privacy_settings(&mut self, ui: &mut egui::Ui) {
        group_title(
            ui,
            "Recording exclusion",
            "Keeps Oxide out of screen recordings and screen sharing.",
        );
        form_group(ui, |ui| {
            let hint = if platform::exclusion_supported() {
                "Applies immediately · best effort"
            } else {
                "Not available on this platform"
            };
            form_row(ui, "Hide Oxide from capture", hint, |ui| {
                self.privacy_control(ui)
            });
        });
        ui.add_space(18.0);
        group_title(
            ui,
            "Never capture",
            "Captures are skipped while the active app or window title contains one of these.",
        );
        form_group(ui, |ui| {
            let mut remove = None;
            for (index, entry) in self.draft.capture_deny.iter().enumerate() {
                form_row(ui, entry, "", |ui| {
                    if quiet(ui, "Remove").clicked() {
                        remove = Some(index);
                    }
                });
                ui.separator();
            }
            if let Some(index) = remove {
                self.draft.capture_deny.remove(index);
            }
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut self.deny_input)
                        .hint_text("App or window title")
                        .desired_width(ui.available_width() - 80.0)
                        .margin(egui::vec2(10.0, 7.0)),
                );
                let submitted =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.button("Add").clicked() || submitted) && !self.deny_input.trim().is_empty() {
                    self.draft
                        .capture_deny
                        .push(self.deny_input.trim().chars().take(128).collect());
                    self.deny_input.clear();
                }
            });
        });
        ui.add_space(6.0);
        caption(
            ui,
            "Matching needs the active window's name, which Wayland does not provide.",
        );
    }

    fn storage_settings(&mut self, ui: &mut egui::Ui) {
        group_title(
            ui,
            "Storage",
            "Originals stay in the folders you choose. Nothing is uploaded.",
        );
        form_group(ui, |ui| {
            let destination = self.draft.capture_directory.display().to_string();
            form_row(ui, "Capture destination", &destination, |ui| {
                if ui.button("Change…").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.draft.capture_directory = path;
                }
            });
        });
        ui.add_space(18.0);
        group_title(
            ui,
            "Screenshot folders",
            "Watched folders appear as collections in your sidebar.",
        );
        form_group(ui, |ui| {
            let mut remove = None;
            for (index, folder) in self.draft.folders.iter().enumerate() {
                let name = folder
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                form_row(ui, &name, &folder.display().to_string(), |ui| {
                    if quiet(ui, "Remove").clicked() {
                        remove = Some(index);
                    }
                });
                ui.separator();
            }
            if let Some(index) = remove {
                self.draft.folders.remove(index);
            }
            if ui.button("Add folder…").clicked()
                && let Some(path) = rfd::FileDialog::new().pick_folder()
            {
                let path = std::fs::canonicalize(&path).unwrap_or(path);
                if !self.draft.folders.contains(&path) {
                    let mut candidate = self.draft.clone();
                    candidate.folders.push(path.clone());
                    match candidate
                        .validate()
                        .and_then(|()| self.paths.ensure_original_location(&path))
                    {
                        Ok(()) => self.draft = candidate,
                        Err(error) => self.notice = Some(format!("{error:#}")),
                    }
                }
            }
        });
        ui.add_space(12.0);
        if quiet(ui, "Open data folder").clicked()
            && let Err(e) = open::that(&self.paths.data)
        {
            self.notice = Some(e.to_string());
        }
    }

    fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        group_title(
            ui,
            "Global shortcuts",
            "Work even when another app is focused.",
        );
        form_group(ui, |ui| {
            let rows = [
                ("Summon / hide", &mut self.draft.summon_shortcut),
                ("Hide all", &mut self.draft.hide_shortcut),
                ("Capture region", &mut self.draft.capture_shortcut),
                ("Capture display", &mut self.draft.display_shortcut),
                ("Copy text from region", &mut self.draft.text_shortcut),
                ("New note", &mut self.draft.note_shortcut),
            ];
            let n = rows.len();
            for (i, (label, value)) in rows.into_iter().enumerate() {
                form_row(ui, label, "", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(value)
                            .font(egui::FontId::monospace(14.0))
                            .desired_width(190.0)
                            .margin(egui::vec2(10.0, 6.0)),
                    );
                });
                if i + 1 < n {
                    ui.separator();
                }
            }
        });
        ui.add_space(18.0);
        group_title(ui, "In the app", "");
        form_group(ui, |ui| {
            let rows = [
                ("Search", "Ctrl/Cmd + K  or  F"),
                ("New note", "Ctrl/Cmd + N"),
                ("Save now", "Ctrl/Cmd + S"),
                ("Close panel / hide", "Esc"),
            ];
            for (i, (label, keys)) in rows.into_iter().enumerate() {
                form_row(ui, label, "", |ui| {
                    ui.label(RichText::new(keys).monospace().color(Palette::of(ui).muted));
                });
                if i + 1 < rows.len() {
                    ui.separator();
                }
            }
        });
    }

    fn ocr_settings(&mut self, ui: &mut egui::Ui) {
        group_title(
            ui,
            "Local OCR",
            "Text recognition runs in a separate process on this device.",
        );
        form_group(ui, |ui| {
            form_row(ui, "Pause background OCR", "", |ui| {
                ui.checkbox(&mut self.draft.worker_paused, "");
            });
            ui.separator();
            form_row(ui, "CPU threads", "", |ui| {
                ui.add(egui::DragValue::new(&mut self.draft.threads).range(1..=16));
            });
            ui.separator();
            let models = self.paths.models().display().to_string();
            form_row(ui, "Reader", &models, |ui| {
                if ui.button("Reindex").clicked() {
                    self.send(Request::Reindex(None));
                }
                if ui.button("Restart").clicked()
                    && let Err(e) = self.worker.restart()
                {
                    self.notice = Some(format!("{e:#}"));
                }
            });
        });
        ui.add_space(18.0);
        group_title(
            ui,
            "Maintenance",
            "Notes and original images are kept separate from caches.",
        );
        form_group(ui, |ui| {
            form_row(
                ui,
                "Back up database",
                "Images are not included; back up your capture folder too",
                |ui| {
                    if ui.button("Back up…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_file_name("oxide-backup.db")
                            .save_file()
                    {
                        self.send(Request::Backup(path));
                    }
                },
            );
            ui.separator();
            form_row(
                ui,
                "Clear derived cache",
                "OCR text and thumbnails are rebuilt",
                |ui| {
                    if ui.button("Clear…").clicked() {
                        self.confirmation = Some(Confirmation::Cache);
                    }
                },
            );
        });
    }
}
