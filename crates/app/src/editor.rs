use std::time::Instant;

use oxide_core::Note;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveState {
    NoNote,
    ReadOnly,
    Saving,
    Failed,
    Unsaved,
    Saved,
}

impl SaveState {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoNote => "No note open",
            Self::ReadOnly => "Note in trash · read only",
            Self::Saving => "Saving…",
            Self::Failed => "Save failed · buffer retained",
            Self::Unsaved => "Unsaved changes",
            Self::Saved => "Saved locally",
        }
    }
}

#[derive(Default)]
pub struct Editor {
    pub note: Option<Note>,
    pub generation: u64,
    pub saved_generation: u64,
    pub in_flight: Option<u64>,
    pub changed_at: Option<Instant>,
    pub error: Option<String>,
}

impl Editor {
    pub fn state(&self) -> SaveState {
        if self.note.is_none() {
            SaveState::NoNote
        } else if self
            .note
            .as_ref()
            .is_some_and(|note| note.deleted_at.is_some())
        {
            SaveState::ReadOnly
        } else if self.error.is_some() {
            SaveState::Failed
        } else if self.in_flight.is_some() {
            SaveState::Saving
        } else if self.dirty() {
            SaveState::Unsaved
        } else {
            SaveState::Saved
        }
    }
    pub fn load(&mut self, note: Note) {
        self.note = Some(note);
        self.generation = 0;
        self.saved_generation = 0;
        self.in_flight = None;
        self.changed_at = None;
        self.error = None;
    }
    pub fn changed(&mut self) {
        self.generation += 1;
        self.changed_at = Some(Instant::now());
        self.error = None;
    }
    pub fn dirty(&self) -> bool {
        self.generation != self.saved_generation
    }
    pub fn begin_save(&mut self) -> Option<(u64, Note)> {
        if !self.dirty() || self.in_flight.is_some() {
            return None;
        }
        let note = self.note.clone()?;
        self.in_flight = Some(self.generation);
        Some((self.generation, note))
    }
    pub fn saved(&mut self, generation: u64, saved: &Note) {
        if self.note.as_ref().is_some_and(|note| note.id == saved.id)
            && self.in_flight == Some(generation)
        {
            if let Some(note) = &mut self.note {
                note.revision = saved.revision;
                note.updated_at = saved.updated_at;
            }
            self.saved_generation = generation;
            self.in_flight = None;
            self.error = None;
        }
    }
    pub fn failed(&mut self, error: String) {
        self.in_flight = None;
        self.error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledgement_does_not_discard_edits_made_while_saving() {
        let mut editor = Editor::default();
        editor.load(Note {
            id: 1,
            title: "Note".into(),
            body: String::new(),
            revision: 0,
            updated_at: 0,
            deleted_at: None,
        });
        editor.note.as_mut().unwrap().body = "first".into();
        editor.changed();
        let (generation, mut snapshot) = editor.begin_save().unwrap();
        editor.note.as_mut().unwrap().body = "second".into();
        editor.changed();
        snapshot.revision = 1;
        editor.saved(generation, &snapshot);
        assert_eq!(editor.note.as_ref().unwrap().body, "second");
        assert!(editor.dirty());
        let (generation, mut snapshot) = editor.begin_save().unwrap();
        snapshot.revision = 2;
        editor.saved(generation, &snapshot);
        assert!(!editor.dirty());
    }
    #[test]
    fn failed_save_preserves_buffer_and_allows_retry() {
        let mut editor = Editor::default();
        editor.load(Note {
            id: 1,
            title: "Note".into(),
            body: "important".into(),
            revision: 0,
            updated_at: 0,
            deleted_at: None,
        });
        editor.changed();
        editor.begin_save();
        editor.failed("Disk full".into());
        assert_eq!(editor.note.as_ref().unwrap().body, "important");
        assert!(editor.begin_save().is_some());
    }
}
