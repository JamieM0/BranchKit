//! Minimal error DTO for this prompt's commands. ARCHITECTURE.md §9 specifies a full
//! `AppError { user_message, suggestion, raw, kind }` catalog with stderr-pattern translation —
//! that catalog is built in a later prompt. For now this carries just enough for the frontend to
//! show a message, and every conversion funnels through here so §9 can be dropped in later
//! without touching call sites.

use serde::Serialize;

use crate::git::exec::GitError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub user_message: String,
    pub raw: String,
}

impl AppError {
    pub fn new(user_message: impl Into<String>, raw: impl Into<String>) -> Self {
        Self {
            user_message: user_message.into(),
            raw: raw.into(),
        }
    }
}

impl From<GitError> for AppError {
    fn from(e: GitError) -> Self {
        Self {
            user_message: e.to_string(),
            raw: e.stderr.clone(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self {
            user_message: e.to_string(),
            raw: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            user_message: "Could not read BranchKit's saved settings".to_string(),
            raw: e.to_string(),
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self {
            user_message: "BranchKit hit an internal error".to_string(),
            raw: e.to_string(),
        }
    }
}

impl From<notify::Error> for AppError {
    fn from(e: notify::Error) -> Self {
        Self {
            user_message: "Could not watch this repository for changes".to_string(),
            raw: e.to_string(),
        }
    }
}
