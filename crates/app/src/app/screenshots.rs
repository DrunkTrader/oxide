use super::*;

impl App {
    pub(super) fn capture_controls(&mut self, ui: &mut egui::Ui) {
        caption(ui, "Capture source");
        egui::ComboBox::from_id_salt("capture-display")
            .selected_text(
                self.displays
                    .get(self.display_index)
                    .map_or("No display", |display| display.name.as_str()),
            )
            .show_ui(ui, |ui| {
                for (index, display) in self.displays.iter().enumerate() {
                    ui.selectable_value(&mut self.display_index, index, &display.name);
                }
            });
        ui.add_enabled_ui(!self.capture_pending && self.capture.is_none(), |ui| {
            if ui.button("Capture region").clicked() {
                self.start_capture(true);
                ui.close();
            }
            if ui.button("Capture display").clicked() {
                self.start_capture(false);
                ui.close();
            }
        });
    }

    pub(super) fn library(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            section(
                ui,
                if self.all_view {
                    "Your workspace"
                } else {
                    "Screenshots"
                },
                if self.query.is_empty() {
                    "Keep the original. Find the details later."
                } else {
                    "Results from your local library"
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("Add image", |ui| self.image_actions(ui, None));
                self.filters(ui);
            });
        });
        ui.add_space(18.0);
        if !self.selected_shots.is_empty() {
            ui.horizontal(|ui| {
                caption(ui, format!("{} selected", self.selected_shots.len()));
                let danger = Palette::of(ui).danger;
                if ui
                    .add(
                        egui::Button::new(RichText::new("Move to trash…").color(danger))
                            .frame(false),
                    )
                    .clicked()
                {
                    self.confirmation = Some(Confirmation::Shots(
                        self.selected_shots.iter().copied().collect(),
                    ));
                }
                if quiet(ui, "Clear selection").clicked() {
                    self.selected_shots.clear();
                }
            });
            ui.add_space(12.0);
        }
        if self.all_view {
            ui.label(RichText::new("Notes").size(16.0).strong());
            ui.add_space(6.0);
            let mut chosen = None;
            egui::ScrollArea::vertical()
                .id_salt("all-notes")
                .max_height(154.0)
                .show(ui, |ui| {
                    for note in &self.notes {
                        if ui
                            .add(
                                egui::Button::new(RichText::new(&note.title).strong())
                                    .frame_when_inactive(false),
                            )
                            .clicked()
                        {
                            chosen = Some(note.id);
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(note.excerpt.replace('\n', " "))
                                    .size(12.0)
                                    .weak(),
                            )
                            .truncate(),
                        );
                    }
                    if self.notes.is_empty() {
                        caption(ui, "No matching notes");
                    }
                });
            if let Some(id) = chosen {
                self.all_view = false;
                self.library = false;
                self.defer(AfterSave::Load(id));
            }
            if self.notes.len() >= self.limit && quiet(ui, "Load more notes").clicked() {
                self.limit += 50;
                self.refresh_pending = false;
                self.refresh();
            }
            ui.add_space(18.0);
            ui.label(RichText::new("Screenshots").size(16.0).strong());
            ui.add_space(8.0);
        }
        if self.shots.is_empty() {
            ui.add_space(40.0);
            section(
                ui,
                if self.query.is_empty() {
                    "A place for your screenshots."
                } else {
                    "No screenshots found."
                },
                if self.query.is_empty() {
                    "Capture a region, import an image or connect a screenshot folder."
                } else {
                    "Try fewer words or a different date or folder."
                },
            );
            ui.add_space(16.0);
            if self.query.is_empty() && primary(ui, "Capture a region").clicked() {
                self.start_capture(true);
            }
            return;
        }
        caption(ui, format!("{} screenshots loaded", self.shots.len()));
        ui.add_space(10.0);
        let columns = (ui.available_width() / 246.0).floor().max(1.0) as usize;
        let rows = self.shots.len().div_ceil(columns);
        egui::ScrollArea::vertical()
            .id_salt("screenshot-library")
            .show_rows(ui, 218.0, rows, |ui, range| {
                for row in range {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 12.0;
                        for index in row * columns..((row + 1) * columns).min(self.shots.len()) {
                            self.shot_tile(ui, &self.shots[index].clone());
                        }
                    });
                }
            });
        if self.shots.len() >= self.limit && quiet(ui, "Load more screenshots").clicked() {
            self.limit += 50;
            self.refresh_pending = false;
            self.refresh();
        }
    }

    fn shot_tile(&mut self, ui: &mut egui::Ui, shot: &Screenshot) {
        let p = Palette::of(ui);
        card(ui).show(ui, |ui| {
            ui.set_width(214.0);
            ui.set_min_height(194.0);
            if let Some((texture, size)) = self.picture(shot.id, false) {
                let response = ui.add(
                    egui::Image::new((texture, size))
                        .max_size(egui::vec2(210.0, 132.0))
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
                    [210.0, 132.0],
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
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let filename = Path::new(&shot.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                ui.add(egui::Label::new(RichText::new(filename).size(12.0)).truncate());
                let mut selected = self.selected_shots.contains(&shot.id);
                if ui
                    .checkbox(&mut selected, "")
                    .on_hover_text("Select screenshot for trash")
                    .changed()
                {
                    if selected {
                        self.selected_shots.insert(shot.id);
                    } else {
                        self.selected_shots.remove(&shot.id);
                    }
                }
            });
            caption(
                ui,
                if shot.available {
                    match shot.ocr_state.as_str() {
                        "ready" => "Text indexed",
                        "pending" => "OCR pending",
                        "no_text" => "No text found",
                        "failed" => "OCR failed",
                        "invalid" => "Invalid image",
                        other => other,
                    }
                } else {
                    "Original unavailable"
                },
            );
            if !shot.available {
                ui.colored_label(p.danger, "Original unavailable");
            }
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
        let p = Palette::new(context.style_of(context.theme()).visuals.dark_mode);
        egui::Window::new("Screenshot detail")
            .id(egui::Id::new("detail-window"))
            .open(&mut open)
            .collapsible(false)
            .frame(egui::Frame::window(&context.style_of(context.theme())).fill(p.raised))
            .default_size([920.0, 650.0])
            .show(context, |ui| {
                self.detail_actions(ui, id, context);
                ui.add_space(8.0);
                if let Some(shot) = &shot {
                    caption(ui, &shot.path);
                    if let Some(time) = chrono::DateTime::from_timestamp_millis(shot.captured_at) {
                        caption(
                            ui,
                            format!(
                                "{} · {}×{} · {}",
                                time.with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d %H:%M:%S"),
                                shot.width,
                                shot.height,
                                match shot.time_source.as_str() {
                                    "captured" => "Recorded capture time",
                                    "modified" => "File modification time at import/recovery",
                                    _ => "Legacy timestamp",
                                }
                            ),
                        );
                    }
                    ui.horizontal(|ui| {
                        if quiet(ui, "Open externally").clicked()
                            && let Err(error) = open::that(&shot.path)
                        {
                            self.notice = Some(format!("Cannot open external viewer: {error}"));
                        }
                        if quiet(ui, "Reveal folder").clicked()
                            && let Some(parent) = Path::new(&shot.path).parent()
                            && let Err(error) = open::that(parent)
                        {
                            self.notice = Some(format!("Cannot reveal folder: {error}"));
                        }
                    });
                    if let Some(error) = &shot.ocr_error {
                        ui.colored_label(p.danger, error);
                    }
                }
                let mut chosen = None;
                ui.horizontal_wrapped(|ui| {
                    for note in &self.linked {
                        if quiet(ui, &format!("Linked note · {}", note.title)).clicked() {
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
            if quiet(ui, "Copy all text").clicked() {
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
            if quiet(ui, "Copy selected").clicked() {
                if selected.is_empty() {
                    self.notice = Some("Select recognized lines first".into());
                } else {
                    context.copy_text(selected.clone());
                }
            }
            if quiet(ui, "Insert selected").clicked() {
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
                && quiet(ui, "Attach to note").clicked()
            {
                self.send(Request::Attach(note, id));
            }
            if quiet(ui, "Retry OCR").clicked() {
                self.send(Request::Reindex(Some(id)));
            }
            if ui
                .add(
                    egui::Button::new(
                        RichText::new("Trash original").color(Palette::of(ui).danger),
                    )
                    .frame(false),
                )
                .clicked()
            {
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
            ui.colored_label(Palette::of(ui).danger, error);
        } else {
            ui.spinner();
            caption(ui, "Loading original…");
        }
        if self.lines.is_empty() {
            caption(
                ui,
                "No recognized text yet. The original remains available independently of OCR.",
            );
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
        egui::Stroke::new(1.5, ACCENT),
        egui::StrokeKind::Inside,
    );
}
