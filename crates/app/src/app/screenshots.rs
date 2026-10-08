use super::*;

impl App {
    pub(super) fn capture_controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("capture-display")
            .selected_text(
                self.displays
                    .get(self.display_index)
                    .map_or("No display", |d| d.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for (index, display) in self.displays.iter().enumerate() {
                    ui.selectable_value(&mut self.display_index, index, &display.name);
                }
            });
        ui.add_enabled_ui(!self.capture_pending && self.capture.is_none(), |ui| {
            if ui.button("Capture region").clicked() {
                self.start_capture(true);
            }
            if ui.button("Capture display").clicked() {
                self.start_capture(false);
            }
        });
    }

    pub(super) fn library(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            self.capture_controls(ui);
            if ui.button("Import image").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                    .pick_file()
            {
                self.send(Request::Import(path, None));
            }
            if ui.button("Paste image").clicked() {
                self.send(Request::Paste(None, self.config.capture_directory.clone()));
            }
            if ui
                .add_enabled(
                    !self.selected_shots.is_empty(),
                    egui::Button::new(format!("Trash selected ({})", self.selected_shots.len())),
                )
                .clicked()
            {
                self.confirmation = Some(Confirmation::Shots(
                    self.selected_shots.iter().copied().collect(),
                ));
            }
        });
        ui.separator();
        if self.all_view {
            ui.label(RichText::new("NOTES").size(12.0).weak());
            let mut chosen = None;
            egui::ScrollArea::vertical()
                .id_salt("all-notes")
                .max_height(180.0)
                .show(ui, |ui| {
                    for note in &self.notes {
                        if ui.link(&note.title).clicked() {
                            chosen = Some(note.id);
                        }
                        ui.label(
                            RichText::new(note.excerpt.replace('\n', " "))
                                .size(12.0)
                                .weak(),
                        );
                    }
                    if self.notes.is_empty() {
                        ui.label("No matching notes");
                    }
                });
            if let Some(id) = chosen {
                self.all_view = false;
                self.library = false;
                self.defer(AfterSave::Load(id));
            }
            if self.notes.len() >= self.limit && ui.button("Load more notes").clicked() {
                self.limit += 50;
                self.refresh_pending = false;
                self.refresh();
            }
            ui.separator();
            ui.label(RichText::new("SCREENSHOTS").size(12.0).weak());
        }
        if self.shots.is_empty() {
            ui.heading(if self.query.is_empty() {
                "Your visual library starts here."
            } else {
                "No matching screenshots"
            });
            ui.label("Capture, import an image, or add an existing screenshot folder in Settings.");
            return;
        }
        let columns = (ui.available_width() / 210.0).floor().max(1.0) as usize;
        let rows = self.shots.len().div_ceil(columns);
        egui::ScrollArea::vertical()
            .id_salt("screenshot-library")
            .show_rows(ui, 190.0, rows, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        for index in row * columns..((row + 1) * columns).min(self.shots.len()) {
                            let shot = self.shots[index].clone();
                            self.shot_tile(ui, &shot);
                        }
                    });
                }
            });
        if self.shots.len() >= self.limit && ui.button("Load more screenshots").clicked() {
            self.limit += 50;
            self.refresh_pending = false;
            self.refresh();
        }
    }

    fn shot_tile(&mut self, ui: &mut egui::Ui, shot: &Screenshot) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(185.0);
            ui.set_min_height(170.0);
            if let Some((texture, size)) = self.picture(shot.id, false) {
                let response = ui.add(
                    egui::Image::new((texture, size))
                        .max_size(egui::vec2(180.0, 120.0))
                        .sense(egui::Sense::click()),
                );
                if let Some(texture) = self.textures.get(&(shot.id, false))
                    && let Ok(query) = Query::parse(&self.query)
                {
                    for line in &texture.lines {
                        if query.matches_line(&line.text) {
                            highlight(ui, line_rect(response.rect, line));
                        }
                    }
                }
                if response.clicked() {
                    self.select_shot(shot.id);
                }
            } else if ui
                .add_sized(
                    [180.0, 120.0],
                    egui::Button::new(if self.image_errors.contains_key(&(shot.id, false)) {
                        "Image unavailable"
                    } else {
                        "Loading…"
                    }),
                )
                .clicked()
            {
                self.select_shot(shot.id);
            }
            let mut selected = self.selected_shots.contains(&shot.id);
            let filename = Path::new(&shot.path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            if ui
                .checkbox(&mut selected, RichText::new(filename).size(11.0))
                .changed()
            {
                if selected {
                    self.selected_shots.insert(shot.id);
                } else {
                    self.selected_shots.remove(&shot.id);
                }
            }
            ui.label(
                RichText::new(if shot.available {
                    &shot.ocr_state
                } else {
                    "Original unavailable"
                })
                .size(11.0)
                .weak(),
            );
        });
    }

    pub(super) fn detail(&mut self, context: &egui::Context) {
        let Some(id) = self.detail else {
            return;
        };
        let shot = self
            .shots
            .iter()
            .chain(self.attachments.iter())
            .find(|shot| shot.id == id)
            .cloned();
        let mut open = true;
        egui::Window::new("Screenshot detail")
            .id(egui::Id::new("detail-window"))
            .open(&mut open)
            .default_size([920.0, 650.0])
            .show(context, |ui| {
                self.detail_actions(ui, id, context);
                if let Some(shot) = &shot {
                    ui.label(RichText::new(&shot.path).size(11.0).weak());
                    if let Some(time) = chrono::DateTime::from_timestamp_millis(shot.captured_at) {
                        ui.label(format!(
                            "{} · {}×{} · {}",
                            time.with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M:%S"),
                            shot.width,
                            shot.height,
                            match shot.time_source.as_str() {
                                "captured" => "Recorded capture time",
                                "modified" => "File modification time at import/recovery",
                                _ => "Legacy timestamp (provenance unknown)",
                            }
                        ));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Open externally (recordable)").clicked()
                            && let Err(error) = open::that(&shot.path)
                        {
                            self.notice = Some(format!("Cannot open external viewer: {error}"));
                        }
                        if ui.button("Reveal folder").clicked()
                            && let Some(parent) = Path::new(&shot.path).parent()
                            && let Err(error) = open::that(parent)
                        {
                            self.notice = Some(format!("Cannot reveal folder: {error}"));
                        }
                    });
                    if let Some(error) = &shot.ocr_error {
                        ui.colored_label(Color32::from_rgb(200, 100, 80), error);
                    }
                }
                let mut chosen = None;
                ui.horizontal_wrapped(|ui| {
                    for note in &self.linked {
                        if ui.link(format!("Linked note: {}", note.title)).clicked() {
                            chosen = Some(note.id);
                        }
                    }
                });
                if let Some(id) = chosen {
                    self.library = false;
                    self.all_view = false;
                    self.defer(AfterSave::Load(id));
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("detail-content")
                    .show(ui, |ui| self.detail_image(ui, id));
            });
        if !open {
            self.detail = None;
            self.selected_lines.clear();
        }
    }

    fn detail_actions(&mut self, ui: &mut egui::Ui, id: i64, context: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Copy all text").clicked() {
                let text = self
                    .lines
                    .iter()
                    .map(|line| line.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    self.notice = Some("No recognized text to copy".into());
                } else {
                    context.copy_text(text);
                }
            }
            let selected = self
                .lines
                .iter()
                .enumerate()
                .filter(|(index, _)| self.selected_lines.contains(index))
                .map(|(_, line)| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if ui.button("Copy selected").clicked() {
                if selected.is_empty() {
                    self.notice = Some("Select recognized lines first".into());
                } else {
                    context.copy_text(selected.clone());
                }
            }
            if ui.button("Insert selected into note").clicked() {
                if selected.is_empty() {
                    self.notice = Some("Select recognized lines first".into());
                } else if let Some(note) = &mut self.editor.note
                    && note.deleted_at.is_none()
                {
                    note.body.push('\n');
                    note.body.push_str(&selected);
                    self.editor.changed();
                } else {
                    self.notice = Some("Open an active note before inserting text".into());
                }
            }
            if let Some(note) = self
                .editor
                .note
                .as_ref()
                .filter(|note| note.deleted_at.is_none())
                .map(|note| note.id)
                && ui.button("Attach to current note").clicked()
            {
                self.send(Request::Attach(note, id));
            }
            if ui.button("Retry OCR").clicked() {
                self.send(Request::Reindex(Some(id)));
            }
            if ui.button("Retry image").clicked() {
                self.image_errors.remove(&(id, true));
                self.textures.remove(&(id, true));
            }
            if ui.button("Trash original").clicked() {
                self.confirmation = Some(Confirmation::Shots(vec![id]));
            }
        });
    }

    fn detail_image(&mut self, ui: &mut egui::Ui, id: i64) {
        if let Some((texture, size)) = self.picture(id, true) {
            let response = ui.add(
                egui::Image::new((texture, size))
                    .max_size(egui::vec2(ui.available_width(), 480.0))
                    .sense(egui::Sense::click_and_drag()),
            );
            let query = Query::parse(&self.query).unwrap_or_default();
            for (index, line) in self.lines.iter().enumerate() {
                let bounds = line_rect(response.rect, line);
                if self.selected_lines.contains(&index) || query.matches_line(&line.text) {
                    highlight(ui, bounds);
                }
                let within = response
                    .interact_pointer_pos()
                    .is_some_and(|point| bounds.contains(point));
                if response.clicked() && within && !self.selected_lines.insert(index) {
                    self.selected_lines.remove(&index);
                }
                if response.dragged() && within {
                    self.selected_lines.insert(index);
                }
            }
        } else if let Some(error) = self.image_errors.get(&(id, true)) {
            ui.colored_label(Color32::from_rgb(200, 100, 80), error);
        } else {
            ui.spinner();
            ui.label("Loading original…");
        }
        if self.lines.is_empty() {
            ui.label("No recognized text yet. The image remains available independently of OCR.");
        }
        for (index, line) in self.lines.iter().enumerate() {
            let mut selected = self.selected_lines.contains(&index);
            if ui.checkbox(&mut selected, &line.text).changed() {
                if selected {
                    self.selected_lines.insert(index);
                } else {
                    self.selected_lines.remove(&index);
                }
            }
        }
    }
}

fn line_rect(image: egui::Rect, line: &Line) -> egui::Rect {
    egui::Rect::from_min_size(
        image.min + egui::vec2(line.x * image.width(), line.y * image.height()),
        egui::vec2(line.w * image.width(), line.h * image.height()),
    )
}

fn highlight(ui: &egui::Ui, bounds: egui::Rect) {
    ui.painter().rect_stroke(
        bounds,
        1.0,
        egui::Stroke::new(1.5, Color32::from_rgb(190, 110, 76)),
        egui::StrokeKind::Inside,
    );
}
