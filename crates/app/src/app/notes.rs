use super::*;

impl App {
    pub(super) fn note_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(if self.trash_view {
                    "Note trash"
                } else {
                    "Notes"
                })
                .size(16.0)
                .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                self.filters(ui);
            });
        });
        caption(ui, format!("{} loaded", self.notes.len()));
        ui.add_space(14.0);
        if self.notes.is_empty() {
            caption(
                ui,
                if !self.query.is_empty() {
                    "No matching notes. Try another search."
                } else if self.trash_view {
                    "No notes in trash."
                } else {
                    "Your first note starts here."
                },
            );
        }
        let current = self.editor.note.as_ref().map(|note| note.id);
        let mut clicked = None;
        let mut restored = None;
        let p = Palette::of(ui);
        egui::ScrollArea::vertical()
            .id_salt("notes-list")
            .show_rows(ui, 82.0, self.notes.len(), |ui, range| {
                for index in range {
                    let note = &self.notes[index];
                    ui.push_id(note.id, |ui| {
                        let title = if note.title.is_empty() {
                            "Untitled note"
                        } else {
                            &note.title
                        };
                        let selected = current == Some(note.id);
                        let button = egui::Button::new(RichText::new(title).strong())
                            .right_text("")
                            .selected(selected)
                            .truncate()
                            .stroke(egui::Stroke::NONE)
                            .fill(if selected {
                                p.accent_soft
                            } else {
                                Color32::TRANSPARENT
                            });
                        if ui.add_sized([ui.available_width(), 30.0], button).clicked() {
                            clicked = Some(note.id);
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(note.excerpt.replace('\n', " "))
                                    .size(12.0)
                                    .color(p.muted),
                            )
                            .truncate(),
                        );
                        if self.trash_view {
                            if quiet(ui, "Restore note").clicked() {
                                restored = Some(note.id);
                            }
                        } else if let Some(date) =
                            chrono::DateTime::from_timestamp_millis(note.updated_at)
                        {
                            caption(
                                ui,
                                date.with_timezone(&chrono::Local)
                                    .format("%b %-d · %H:%M")
                                    .to_string(),
                            );
                        }
                        ui.add_space(8.0);
                    });
                }
            });
        if let Some(id) = clicked {
            self.defer(AfterSave::Load(id));
        }
        if let Some(id) = restored {
            self.send(Request::NoteTrash(id, false));
        }
        if self.notes.len() >= self.limit && quiet(ui, "Load more notes").clicked() {
            self.limit += 50;
            self.refresh_pending = false;
            self.refresh();
        }
    }

    pub(super) fn note_editor(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.editor.note.as_ref().map(|note| note.id) else {
            ui.add_space((ui.available_height() * 0.25).max(32.0));
            section(
                ui,
                "Keep the context.",
                "Notes, screenshots and the text inside them. Saved locally.",
            );
            ui.add_space(16.0);
            if primary(ui, "Create a note").clicked() {
                self.new_note();
            }
            return;
        };
        let trashed = self
            .editor
            .note
            .as_ref()
            .is_some_and(|note| note.deleted_at.is_some());
        ui.horizontal(|ui| {
            caption(
                ui,
                if trashed {
                    "Note trash / Read only"
                } else {
                    "Notes / Editor"
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("Capture", |ui| self.capture_controls(ui));
                ui.menu_button("Actions", |ui| {
                    if ui.button("Duplicate note").clicked() {
                        self.defer(AfterSave::Duplicate(id));
                        ui.close();
                    }
                    if ui.button("Export Markdown…").clicked() {
                        self.export_current();
                        ui.close();
                    }
                    if !trashed
                        && ui
                            .add(egui::Button::new(
                                RichText::new("Move to note trash").color(Palette::of(ui).danger),
                            ))
                            .clicked()
                    {
                        self.confirmation = Some(Confirmation::Note(id));
                        ui.close();
                    }
                });
                ui.menu_button("Add image", |ui| {
                    self.image_actions(ui, (!trashed).then_some(id));
                });
            });
        });
        ui.add_space(18.0);
        let mut changed = false;
        ui.add_enabled_ui(!trashed && !self.switching_note, |ui| {
            if let Some(note) = &mut self.editor.note {
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut note.title)
                            .id(egui::Id::new(("note-title", id)))
                            .hint_text("Untitled note")
                            .font(egui::FontId::proportional(28.0))
                            .frame(egui::Frame::NONE)
                            .margin(egui::vec2(0.0, 4.0))
                            .desired_width(f32::INFINITY),
                    )
                    .changed();
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .id_salt("note-editor")
                    .max_height((ui.available_height() - 154.0).max(90.0))
                    .show(ui, |ui| {
                        changed |= ui
                            .add(
                                egui::TextEdit::multiline(&mut note.body)
                                    .id(egui::Id::new(("note-body", id)))
                                    .hint_text("Write a note…\n\nMarkdown is welcome here.")
                                    .font(egui::FontId::proportional(16.0))
                                    .frame(egui::Frame::NONE)
                                    .margin(egui::vec2(0.0, 4.0))
                                    .desired_rows(18)
                                    .desired_width(f32::INFINITY),
                            )
                            .changed();
                    });
            }
        });
        if changed {
            self.editor.changed();
        }
        if trashed {
            caption(ui, "Restore this note to continue editing.");
        }
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(6.0);
        caption(ui, format!("Attachments · {}", self.attachments.len()));
        if self.attachments.is_empty() {
            caption(
                ui,
                "Capture a region or add an image to keep the evidence with your note.",
            );
            return;
        }
        let mut detach = None;
        egui::ScrollArea::horizontal()
            .id_salt("attachments")
            .show_viewport(ui, |ui, viewport| {
                let (_, space) =
                    ui.allocate_space(egui::vec2(self.attachments.len() as f32 * 170.0, 124.0));
                let start = (viewport.min.x / 170.0).floor().max(0.0) as usize;
                let end = ((viewport.max.x / 170.0).ceil() as usize).min(self.attachments.len());
                for index in start..end {
                    let shot = self.attachments[index].clone();
                    let rect = egui::Rect::from_min_size(
                        space.min + egui::vec2(index as f32 * 170.0, 0.0),
                        egui::vec2(160.0, 124.0),
                    );
                    ui.scope_builder(
                        egui::UiBuilder::new().id_salt(shot.id).max_rect(rect),
                        |ui| {
                            if let Some((texture, size)) = self.picture(shot.id, false) {
                                if ui
                                    .add(
                                        egui::Image::new((texture, size))
                                            .corner_radius(5)
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
                                caption(
                                    ui,
                                    if shot.available {
                                        &shot.ocr_state
                                    } else {
                                        "Unavailable"
                                    },
                                );
                                if !trashed && quiet(ui, "Detach").clicked() {
                                    detach = Some(shot.id);
                                }
                            });
                        },
                    );
                }
            });
        if let Some(shot) = detach {
            self.send(Request::Detach(id, shot));
        }
    }

    pub(super) fn image_actions(&mut self, ui: &mut egui::Ui, note: Option<i64>) {
        if ui.button("Import image…").clicked()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Import image")
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
        {
            self.send(Request::Import(path, note));
            ui.close();
        }
        if ui.button("Paste image").clicked() {
            self.send(Request::Paste(note, self.config.capture_directory.clone()));
            ui.close();
        }
    }
}
