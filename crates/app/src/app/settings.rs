use super::*;

impl App {
    pub(super) fn filters(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Filters",|ui| {
            ui.label("Local calendar day");
            egui::ComboBox::from_id_salt("filter-kind").selected_text(self.filter_kind).show_ui(ui,|ui| {
                for kind in ["date:","after:","before:"]{ui.selectable_value(&mut self.filter_kind,kind,kind);}
            });
            ui.add(egui::TextEdit::singleline(&mut self.filter_date).hint_text("YYYY-MM-DD"));
            if ui.button("Apply day filter").clicked(){
                if self.filter_date.len()==10 && chrono::NaiveDate::parse_from_str(&self.filter_date,"%Y-%m-%d").is_ok(){
                    self.query.push_str(&format!(" {}{}",self.filter_kind,self.filter_date));self.changed_query();ui.close();
                }else{self.notice=Some("Use a complete calendar date in YYYY-MM-DD format".into());}
            }
            if ui.button("Choose screenshot folder…").clicked() && let Some(folder)=rfd::FileDialog::new().pick_folder(){
                let folder=folder.to_string_lossy();
                if folder.contains('"'){self.notice=Some("The quoted folder-filter syntax cannot represent this folder name".into());}
                else{self.query.push_str(&format!(" in:\"{folder}\""));self.library=true;self.all_view=false;self.changed_query();ui.close();}
            }
            ui.label("Folders filter screenshots only. Note dates use last edit; imported-image dates use file modification time.");
            if ui.button("Clear filters").clicked() && let Ok(query)=Query::parse(&self.query){self.query=query.terms.join(" ");self.changed_query();ui.close();}
        });
    }

    fn changed_query(&mut self) {
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
            .open(&mut open)
            .default_size([640.0, 580.0])
            .show(context, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Your local workspace");
                    ui.label("Notes and originals stay on this device. OCR runs locally.");
                    ui.separator();
                    let mut enabled = self.pending_config.as_ref().unwrap_or(&self.config).recording_exclusion_enabled;
                    if ui.add_enabled(platform::exclusion_supported() && self.pending_config.is_none(), egui::Checkbox::new(&mut enabled, "Recording exclusion")).changed() {
                        self.toggle_exclusion(enabled);
                    }
                    ui.label(if platform::exclusion_supported() {
                        "On applies Windows capture exclusion. Compatibility with recorders must be tested."
                    } else {
                        "Recording exclusion is unavailable on this platform. Notes/capture remain usable with it Off."
                    });
                    ui.separator();
                    ui.label("Capture destination");
                    ui.horizontal(|ui| {
                        ui.label(self.draft.capture_directory.display().to_string());
                        if ui.button("Choose…").clicked() && let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.draft.capture_directory = path;
                        }
                    });
                    ui.label("Watched screenshot folders");
                    let mut remove = None;
                    for (index, folder) in self.draft.folders.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(folder.display().to_string());
                            if ui.small_button("Remove").clicked() { remove = Some(index); }
                        });
                    }
                    if let Some(index) = remove { self.draft.folders.remove(index); }
                    if ui.button("Add folder").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_folder()
                        && !self.draft.folders.contains(&path) {
                        self.draft.folders.push(path);
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Appearance");
                        egui::ComboBox::from_id_salt("theme").selected_text(&self.draft.theme).show_ui(ui, |ui| {
                            for theme in ["system", "light", "dark"] { ui.selectable_value(&mut self.draft.theme, theme.into(), theme); }
                        });
                    });
                    ui.checkbox(&mut self.draft.always_on_top, "Always on top");
                    ui.checkbox(&mut self.draft.start_at_login, "Start at login");
                    ui.checkbox(&mut self.draft.worker_paused, "Pause background OCR");
                    ui.horizontal(|ui| {
                        ui.label("OCR threads");
                        ui.add(egui::DragValue::new(&mut self.draft.threads).range(1..=16));
                    });
                    ui.label(format!("Pinned local models: {}",self.paths.models().display()));
                    ui.separator();
                    ui.label("Global shortcuts");
                    egui::Grid::new("shortcuts").show(ui, |ui| {
                        for (label, value) in [
                            ("Summon / hide", &mut self.draft.summon_shortcut),
                            ("Hide all", &mut self.draft.hide_shortcut),
                            ("Capture region", &mut self.draft.capture_shortcut),
                            ("Capture display", &mut self.draft.display_shortcut),
                        ] {
                            ui.label(label);
                            ui.text_edit_singleline(value);
                            ui.end_row();
                        }
                    });
                    if ui.add_enabled(self.pending_config.is_none(), egui::Button::new("Save settings")).clicked() {
                        self.save_preferences(self.draft.clone(), false);
                    }
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Restart reader").clicked() && let Err(e) = self.worker.restart() { self.notice = Some(format!("{e:#}")); }
                        if ui.button("Reindex library").clicked() { self.send(Request::Reindex(None)); }
                        if ui.button("Clear derived cache").clicked() { self.confirmation = Some(Confirmation::Cache); }
                        if ui.button("Back up database").clicked()
                            && let Some(path) = rfd::FileDialog::new().set_file_name("oxide-backup.db").save_file() {
                            self.send(Request::Backup(path));
                        }
                        if ui.button("Open data directory").clicked() && let Err(e) = open::that(&self.paths.data) { self.notice = Some(e.to_string()); }
                    });
                    ui.label(RichText::new("Database backups do not include screenshot originals. Keep the managed capture folder with your backup.").size(12.0).weak());
                });
            });
        if !open {
            self.settings = false;
        }
    }
}
