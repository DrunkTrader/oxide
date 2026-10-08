//! Typed processing states with stable on-disk names and user-facing labels.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OcrState {
    Pending,
    Ready,
    NoText,
    Failed,
    Invalid,
    Trashed,
    Unwatched,
    Unknown,
}

impl OcrState {
    pub fn from_storage(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "ready" => Self::Ready,
            "no_text" => Self::NoText,
            "failed" => Self::Failed,
            "invalid" => Self::Invalid,
            "trashed" => Self::Trashed,
            "unwatched" => Self::Unwatched,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "OCR pending",
            Self::Ready => "Text indexed",
            Self::NoText => "No text found",
            Self::Failed => "OCR failed · retry available",
            Self::Invalid => "Invalid image",
            Self::Trashed => "Original in system trash",
            Self::Unwatched => "Source no longer watched",
            Self::Unknown => "OCR status unavailable",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerPhase {
    #[default]
    #[serde(alias = "Starting")]
    Starting,
    Scanning,
    #[serde(alias = "Preparing local OCR models")]
    Preparing,
    #[serde(alias = "Recognizing screenshot")]
    Recognizing,
    #[serde(alias = "Paused")]
    Paused,
    #[serde(alias = "Up to date")]
    Ready,
    #[serde(alias = "OCR unavailable")]
    OcrUnavailable,
    #[serde(
        alias = "Reader not started",
        alias = "Reader stopped; cached search remains available"
    )]
    Stopped,
    #[serde(alias = "Reader status unavailable")]
    Unavailable,
}

impl WorkerPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting reader…",
            Self::Scanning => "Scanning screenshot sources…",
            Self::Preparing => "Preparing local OCR models…",
            Self::Recognizing => "Recognizing screenshot…",
            Self::Paused => "Reader paused",
            Self::Ready => "Reader up to date",
            Self::OcrUnavailable => "OCR unavailable · cached search works",
            Self::Stopped => "Reader stopped · cached search works",
            Self::Unavailable => "Reader status unavailable",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceHealth {
    pub path: std::path::PathBuf,
    pub available: bool,
    pub watched: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_reader_status_remains_readable() -> anyhow::Result<()> {
        let old = r#"{"state":"Up to date","error":null,"indexed":1,"backlog":0,"updated_at":42}"#;
        let status: crate::WorkerStatus = serde_json::from_str(old)?;
        assert_eq!(status.state, WorkerPhase::Ready);
        assert!(status.sources.is_empty());
        assert_eq!(serde_json::to_value(status)?["state"], "ready");
        Ok(())
    }
}
