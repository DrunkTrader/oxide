use super::*;

impl App {
    pub(super) fn note_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("+ New note").clicked() {
                self.defer(AfterSave::New);
            }
            if ui.selectable_label(self.trash_view, "Trash").clicked() {
                self.trash_view = !self.trash_view;
                self.query_generation += 1;
                self.refresh_pending = false;
                self.refresh();
            }
        });
        ui.separator();
        if self.notes.is_empty() {
            ui.label(if !self.query.is_empty() {
                "No matching notes"
            } else if self.trash_view {
                "Note trash is empty"
            } else {
                "Your notes live here. Create one to start."
            });
        }
        let current = self.editor.note.as_ref().map(|note| note.id);
        let mut clicked = None;
        let mut restored = None;
        egui::ScrollArea::vertical()
            .id_salt("notes-list")
            .show_rows(ui, 72.0, self.notes.len(), |ui, range| {
                for index in range {
                    let note = &self.notes[index];
                    ui.push_id(note.id, |ui| {
                        let title = if note.title.is_empty() {
                            "Untitled note"
                        } else {
                            &note.title
                        };
                        if ui
                            .selectable_label(
                                current == Some(note.id),
                                RichText::new(title).strong(),
                            )
                            .clicked()
                        {
                            clicked = Some(note.id);
                        }
                        ui.label(
                            RichText::new(note.excerpt.replace('\n', " "))
                                .size(12.0)
                                .weak(),
                        );
                        if self.trash_view && ui.small_button("Restore").clicked() {
                            restored = Some(note.id);
                        }
                    });
                    ui.separator();
                }
            });
        if let Some(id) = clicked {
            self.defer(AfterSave::Load(id));
        }
        if let Some(id) = restored {
            self.send(Request::NoteTrash(id, false));
        }
        if self.notes.len() >= self.limit && ui.button("Load more notes").clicked() {
            self.limit += 50;
            self.refresh_pending = false;
            self.refresh();
        }
    }

    pub(super) fn note_editor(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.editor.note.as_ref().map(|note| note.id) else {
            ui.add_space(48.0);
            ui.heading("A place for what matters.");
            ui.label("Write a note. Capture the context. Find it later.");
            if ui.button("Create your first note").clicked() {
                self.defer(AfterSave::New);
            }
            return;
        };
        let trashed = self
            .editor
            .note
            .as_ref()
            .is_some_and(|note| note.deleted_at.is_some());
        ui.horizontal_wrapped(|ui| {
            self.capture_controls(ui);
            if ui.button("Import image").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_title("Import image")
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                    .pick_file()
            {
                self.send(Request::Import(path, (!trashed).then_some(id)));
            }
            if ui.button("Paste image").clicked() {
                self.send(Request::Paste(
                    (!trashed).then_some(id),
                    self.config.capture_directory.clone(),
                ));
            }
            if ui.button("Duplicate").clicked() {
                self.defer(AfterSave::Duplicate(id));
            }
            if ui.button("Export").clicked() {
                self.export_current();
            }
            if !trashed && ui.button("Trash note").clicked() {
                self.confirmation = Some(Confirmation::Note(id));
            }
        });
        ui.separator();
        let mut changed = false;
        ui.add_enabled_ui(!trashed && !self.switching_note,|ui| {
            if let Some(note)=&mut self.editor.note {
                let title=egui::TextEdit::singleline(&mut note.title).id(egui::Id::new(("note-title",id)))
                    .hint_text("Untitled note").font(egui::FontId::proportional(27.0)).desired_width(f32::INFINITY);
                changed|=ui.add(title).changed();
                egui::ScrollArea::vertical().id_salt("note-editor").max_height((ui.available_height()-160.0).max(120.0)).show(ui,|ui| {
                    let body=egui::TextEdit::multiline(&mut note.body).id(egui::Id::new(("note-body",id)))
                        .hint_text("Write freely. Markdown syntax works here.\n\nCapture a region to attach visual context.")
                        .desired_rows(16).desired_width(f32::INFINITY);
                    changed|=ui.add(body).changed();
                });
            }
        });
        if changed {
            self.editor.changed();
        }
        if trashed {
            ui.label("This note is in trash. Restore it to edit.");
        }
        ui.separator();
        ui.label(
            RichText::new(format!("ATTACHMENTS · {}", self.attachments.len()))
                .size(12.0)
                .weak(),
        );
        let mut detach = None;
        egui::ScrollArea::horizontal()
            .id_salt("attachments")
            .show_viewport(ui, |ui, viewport| {
                let (_, space) =
                    ui.allocate_space(egui::vec2(self.attachments.len() as f32 * 170.0, 120.0));
                let start = (viewport.min.x / 170.0).floor().max(0.0) as usize;
                let end = ((viewport.max.x / 170.0).ceil() as usize).min(self.attachments.len());
                for index in start..end {
                    let shot = self.attachments[index].clone();
                    let rect = egui::Rect::from_min_size(
                        space.min + egui::vec2(index as f32 * 170.0, 0.0),
                        egui::vec2(160.0, 120.0),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        ui.vertical(|ui| {
                            if let Some((texture, size)) = self.picture(shot.id, false) {
                                if ui
                                    .add(
                                        egui::Image::new((texture, size))
                                            .max_size(egui::vec2(150.0, 85.0))
                                            .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    self.select_shot(shot.id);
                                }
                            } else if ui
                                .button(if shot.available {
                                    "Loading image…"
                                } else {
                                    "Original unavailable"
                                })
                                .clicked()
                            {
                                self.select_shot(shot.id);
                            }
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(if shot.available {
                                        &shot.ocr_state
                                    } else {
                                        "Original unavailable"
                                    })
                                    .size(11.0),
                                );
                                if !trashed && ui.small_button("Detach").clicked() {
                                    detach = Some(shot.id);
                                }
                            });
                        });
                    });
                }
            });
        if let Some(shot) = detach {
            self.send(Request::Detach(id, shot));
        }
    }
}
