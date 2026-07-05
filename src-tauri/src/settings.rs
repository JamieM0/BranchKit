//! Persisted app settings — DESIGN_SPEC.md §13. One JSON file in the app config dir, written
//! instantly on every change from the frontend (no Save button, ARCHITECTURE.md §2's "config
//! files store only non-secret settings"). This file never contains a secret: the AI section's API
//! key and the GitHub token both live in the OS keychain (`credentials.rs`) — only the non-secret
//! shape of each (provider, base URL, model name…) is persisted here.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PullMode {
    #[default]
    Ff,
    Rebase,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    /// Minutes between auto-fetch ticks; `0` = off. DESIGN_SPEC §13 offers off/1/5/15.
    #[serde(default = "default_auto_fetch")]
    pub auto_fetch_interval_minutes: u32,
    #[serde(default)]
    pub open_last_repos_on_launch: bool,
    #[serde(default)]
    pub default_clone_dir: Option<String>,
}

fn default_auto_fetch() -> u32 {
    1
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            auto_fetch_interval_minutes: default_auto_fetch(),
            open_last_repos_on_launch: false,
            default_clone_dir: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    /// "system" | "dark" | "light" — kept a plain string so it stays in lockstep with the
    /// frontend's existing `theme.svelte.ts` store rather than duplicating that enum.
    pub theme: String,
    /// "comfortable" (28px) | "compact" (24px) — persisted for forward-compatibility; only
    /// Comfortable is actually wired into the graph's row geometry today (geometry.ts's own
    /// comment already defers Compact to a later prompt).
    pub graph_density: String,
    /// "relative" | "absolute".
    pub date_style: String,
    pub show_avatars: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            graph_density: "comfortable".to_string(),
            date_style: "relative".to_string(),
            show_avatars: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitSettings {
    pub default_pull_mode: PullMode,
    pub push_tags_with_commits: bool,
    pub prune_on_fetch: bool,
    /// DESIGN_SPEC §5/§15.26 — de-noises the LOCAL/REMOTES panel; default ON.
    pub combine_tracking_branches: bool,
    pub commit_summary_guide_length: u32,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            default_pull_mode: PullMode::Ff,
            push_tags_with_commits: false,
            prune_on_fetch: true,
            combine_tracking_branches: true,
            commit_summary_guide_length: 72,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AiProviderKind {
    #[default]
    Local,
    Ollama,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum RemoteApiFormat {
    #[default]
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum CommitStyle {
    #[default]
    Plain,
    Conventional,
}

/// DESIGN_SPEC §13's AI section. Every field here renders in Settings today; wiring an actual
/// provider (downloading the local model, calling Ollama/remote APIs) is out of scope until a
/// later prompt — the master switch and provider fields simply have nothing listening yet. The
/// remote API key itself is never in this struct: it lives in the keychain under
/// `credentials::AI_API_KEY_ACCOUNT`, looked up separately by whatever later prompt wires the
/// provider up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub enabled: bool,
    pub provider: AiProviderKind,
    pub ollama_base_url: String,
    pub ollama_model: Option<String>,
    pub remote_format: RemoteApiFormat,
    pub remote_base_url: String,
    pub remote_model: String,
    pub style: CommitStyle,
    pub max_diff_size_kb: u32,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: AiProviderKind::Local,
            ollama_base_url: "http://localhost:11434".to_string(),
            ollama_model: None,
            remote_format: RemoteApiFormat::OpenAi,
            remote_base_url: String::new(),
            remote_model: String::new(),
            style: CommitStyle::Plain,
            max_diff_size_kb: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppSettings {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub git: GitSettings,
    #[serde(default)]
    pub ai: AiSettings,
}

fn settings_path(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    let dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<AppSettings, AppError> {
    let path = settings_path(&app)?;
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(AppSettings::default());
    };
    Ok(serde_json::from_str(&text).unwrap_or_default())
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: AppSettings) -> Result<(), AppError> {
    let path = settings_path(&app)?;
    let text = serde_json::to_string_pretty(&settings)?;
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_design_spec() {
        let s = AppSettings::default();
        assert_eq!(s.general.auto_fetch_interval_minutes, 1);
        assert!(s.git.combine_tracking_branches);
        assert_eq!(s.git.commit_summary_guide_length, 72);
        assert!(!s.ai.enabled);
        assert_eq!(s.ai.ollama_base_url, "http://localhost:11434");
    }

    #[test]
    fn round_trips_through_json() {
        let s = AppSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults_for_forward_compat() {
        let back: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(back, AppSettings::default());
    }
}
