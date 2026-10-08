//! Local authored content and rebuildable screenshot intelligence.

pub mod config;
pub mod media;
pub mod query;
pub mod status;
pub mod store;

pub use status::{OcrState, SourceHealth, WorkerPhase};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub updated_at: i64,
    pub revision: i64,
    pub deleted_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteSummary {
    pub id: i64,
    pub title: String,
    pub updated_at: i64,
    pub excerpt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Screenshot {
    pub id: i64,
    pub path: String,
    pub managed: bool,
    pub captured_at: i64,
    pub time_source: String,
    pub width: u32,
    pub height: u32,
    pub digest: String,
    pub available: bool,
    pub ocr_state: String,
    pub ocr_error: Option<String>,
    pub updated_at: i64,
}

impl Screenshot {
    pub fn processing_state(&self) -> OcrState {
        OcrState::from_storage(&self.ocr_state)
    }

    pub fn status_label(&self) -> &'static str {
        if self.available || self.processing_state() == OcrState::Trashed {
            self.processing_state().label()
        } else {
            "Original unavailable"
        }
    }

    pub fn time_label(&self) -> &'static str {
        match self.time_source.as_str() {
            "captured" => "Recorded capture time",
            "modified" => "File modification time at import/recovery",
            _ => "Legacy timestamp",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// None when the engine does not expose recognition confidence.
    pub confidence: Option<f32>,
}

impl Line {
    pub fn valid(&self) -> bool {
        [self.x, self.y, self.w, self.h]
            .iter()
            .all(|v| v.is_finite())
            && self.x >= 0.0
            && self.y >= 0.0
            && self.w > 0.0
            && self.h > 0.0
            && self.x + self.w <= 1.001
            && self.y + self.h <= 1.001
            && self
                .confidence
                .is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v))
    }
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub state: WorkerPhase,
    pub error: Option<String>,
    pub indexed: usize,
    pub backlog: usize,
    pub updated_at: i64,
    #[serde(default)]
    pub sources: Vec<SourceHealth>,
    #[serde(default)]
    pub reconciled_at: Option<i64>,
    #[serde(default)]
    pub last_ocr_ms: Option<u64>,
}
