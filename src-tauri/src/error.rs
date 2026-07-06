//! Error translation layer — ARCHITECTURE.md §9. Every `GitError` funnels through
//! `AppError::from` here, which matches its stderr against a catalog of known git failures and
//! attaches a plain-language `user_message` plus an optional actionable `suggestion` the frontend
//! renders as a single button (DESIGN_SPEC.md §11). Unknown errors fall back to a generic
//! sentence; the raw stderr is always carried (scrubbed of any credential userinfo) for the
//! toast/dialog's "Details" expander.

use serde::Serialize;

use crate::git::exec::GitError;

/// A suggested next action attached to a translated error — DESIGN_SPEC.md §11. `action_id` is an
/// opaque string the frontend switches on (it owns the catalog of what each id does, e.g. running
/// a pull, opening credential settings, or the stash-and-checkout compound action) — kept a plain
/// string rather than an enum so new suggestions don't require a Rust changes in lockstep with the
/// frontend catalog.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub label: String,
    pub action_id: String,
}

impl Suggestion {
    fn new(label: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action_id: action_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub user_message: String,
    pub suggestion: Option<Suggestion>,
    pub raw: String,
}

impl AppError {
    pub fn new(user_message: impl Into<String>, raw: impl Into<String>) -> Self {
        Self {
            user_message: user_message.into(),
            suggestion: None,
            raw: scrub(&raw.into()),
        }
    }

    pub fn with_suggestion(
        user_message: impl Into<String>,
        raw: impl Into<String>,
        suggestion: Suggestion,
    ) -> Self {
        Self {
            user_message: user_message.into(),
            suggestion: Some(suggestion),
            raw: scrub(&raw.into()),
        }
    }
}

/// Strips credentials embedded in any `scheme://user:pass@host` URL found in `text` — the "never
/// credentials" half of ARCHITECTURE.md §9's Details rule. Kept a plain string scan (no regex
/// crate dependency beyond what `exec.rs` already needs) since the shape is fixed and narrow.
fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    loop {
        let Some(scheme_at) = rest.find("://") else {
            out.push_str(rest);
            break;
        };
        let (before_scheme, after_marker) = rest.split_at(scheme_at + 3);
        out.push_str(before_scheme);
        // Userinfo is everything up to the next `@` before the next `/` — if there's no `@`
        // before the next slash (or no slash at all), there's no userinfo to scrub.
        let slash = after_marker.find('/').unwrap_or(after_marker.len());
        let at = after_marker[..slash].find('@');
        match at {
            Some(at_idx) => {
                out.push_str("***@");
                rest = &after_marker[at_idx + 1..];
            }
            None => {
                rest = after_marker;
            }
        }
    }
    out
}

/// Matches `stderr` against the ARCHITECTURE.md §9 starter catalog, in priority order. Falls back
/// to `None` for anything unrecognized, letting the caller supply a generic sentence.
fn translate(stderr: &str) -> Option<(String, Suggestion)> {
    let lower = stderr.to_lowercase();

    if lower.contains("could not resolve host") {
        return Some((
            "BranchKit can't reach the network right now".to_string(),
            // Distinct from the plain "retry" id below so the frontend can flip the global
            // offline indicator specifically for this case (and retry on focus, ARCHITECTURE.md
            // §9/§14) rather than for every "Retry"-suggested error.
            Suggestion::new("Retry", "retry-offline"),
        ));
    }
    if lower.contains("does not appear to be a git repository")
        || lower.contains("no configured push destination")
        || (lower.contains("could not read from remote repository")
            && !lower.contains("permission denied"))
    {
        return Some((
            "This repository has no reachable remote — add one (e.g. `git remote add origin <url>`) \
             or check the remote's URL"
                .to_string(),
            Suggestion::new("Show details", "details"),
        ));
    }
    if lower.contains("non-fast-forward") || lower.contains("fetch first") {
        return Some((
            "The remote has commits you don't have yet".to_string(),
            Suggestion::new("Pull first", "pull"),
        ));
    }
    if lower.contains("index.lock") {
        return Some((
            "Another git process is running (editor?)".to_string(),
            Suggestion::new("Retry", "retry"),
        ));
    }
    // Must be checked before the generic auth-failure branch below: this isn't a bad-credential
    // problem (the username/password dialog can't fix it), it's a missing OAuth scope — GitHub
    // hard-rejects any push that touches `.github/workflows/*` unless the token has `workflow`
    // scope (github/mod.rs's SPEC-DEVIATION comment). Tokens issued before that fix was added
    // don't gain the scope retroactively, so the fix is reconnecting, not re-entering a password.
    if lower.contains("refusing to allow an oauth app") && lower.contains("workflow") {
        return Some((
            "GitHub needs an extra permission to push changes to this repo's workflow files"
                .to_string(),
            Suggestion::new("Reconnect GitHub", "reconnect-github"),
        ));
    }
    if lower.contains("authentication failed") || lower.contains("403") {
        return Some((
            "GitHub rejected your credentials".to_string(),
            Suggestion::new("Open credential settings", "open-credentials-settings"),
        ));
    }
    if lower.contains("would be overwritten by checkout") || lower.contains("would be overwritten by merge") {
        return Some((
            "You have uncommitted changes that would be overwritten".to_string(),
            Suggestion::new("Stash and continue", "stash-and-checkout"),
        ));
    }
    if lower.contains("no upstream") || lower.contains("has no upstream branch") {
        return Some((
            "This branch has no upstream to push to".to_string(),
            Suggestion::new("Publish", "publish"),
        ));
    }
    if lower.contains("refusing to merge unrelated histories") {
        return Some((
            "These branches don't share any history".to_string(),
            Suggestion::new("Allow unrelated histories", "allow-unrelated"),
        ));
    }
    None
}

impl From<GitError> for AppError {
    fn from(e: GitError) -> Self {
        let raw = e.stderr.clone();
        match translate(&e.stderr) {
            Some((user_message, suggestion)) => {
                Self::with_suggestion(user_message, raw, suggestion)
            }
            None => Self::new(e.to_string(), raw),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::new(e.to_string(), e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::new("Could not read BranchKit's saved settings", e.to_string())
    }
}

impl From<tauri::Error> for AppError {
    fn from(e: tauri::Error) -> Self {
        Self::new("BranchKit hit an internal error", e.to_string())
    }
}

impl From<notify::Error> for AppError {
    fn from(e: notify::Error) -> Self {
        Self::new("Could not watch this repository for changes", e.to_string())
    }
}

impl From<keyring::Error> for AppError {
    fn from(e: keyring::Error) -> Self {
        Self::new("Could not access your OS keychain", e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        Self::new("Could not reach GitHub", e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::exec::GitErrorKind;

    fn git_error(stderr: &str) -> GitError {
        GitError {
            code: Some(1),
            stderr: stderr.to_string(),
            cmd_summary: "git push".to_string(),
            kind: GitErrorKind::NonZeroExit,
        }
    }

    #[test]
    fn maps_non_fast_forward_to_pull_suggestion() {
        let err: AppError = git_error("! [rejected] main -> main (non-fast-forward)").into();
        assert_eq!(err.suggestion.as_ref().unwrap().action_id, "pull");
    }

    #[test]
    fn maps_fetch_first_to_pull_suggestion() {
        let err: AppError = git_error("Updates were rejected because the remote contains work that you do\nhint: (e.g. 'git pull ...') before pushing again.\nfetch first").into();
        assert_eq!(err.suggestion.as_ref().unwrap().action_id, "pull");
    }

    #[test]
    fn maps_index_lock_to_retry_suggestion() {
        let err: AppError =
            git_error("fatal: Unable to create '/repo/.git/index.lock': File exists.").into();
        assert_eq!(err.suggestion.as_ref().unwrap().action_id, "retry");
    }

    #[test]
    fn maps_missing_workflow_scope_to_reconnect_suggestion() {
        let err: AppError = git_error(
            "! [remote rejected] main -> main (refusing to allow an OAuth App to create or update \
             workflow `.github/workflows/release.yml` without `workflow` scope)\n\
             error: failed to push some refs to 'https://github.com/o/r.git'",
        )
        .into();
        assert_eq!(
            err.suggestion.as_ref().unwrap().action_id,
            "reconnect-github"
        );
    }

    #[test]
    fn maps_auth_failure_to_credentials_suggestion() {
        let err: AppError = git_error("remote: Authentication failed for 'https://example.com/'").into();
        assert_eq!(
            err.suggestion.as_ref().unwrap().action_id,
            "open-credentials-settings"
        );
    }

    #[test]
    fn maps_403_to_credentials_suggestion() {
        let err: AppError = git_error("remote: Permission denied (403)").into();
        assert_eq!(
            err.suggestion.as_ref().unwrap().action_id,
            "open-credentials-settings"
        );
    }

    #[test]
    fn maps_could_not_resolve_host_to_retry_offline_suggestion() {
        let err: AppError =
            git_error("fatal: unable to access 'https://example.com/': Could not resolve host: example.com").into();
        assert_eq!(err.suggestion.as_ref().unwrap().action_id, "retry-offline");
    }

    #[test]
    fn maps_would_be_overwritten_to_stash_and_checkout_suggestion() {
        let err: AppError = git_error(
            "error: Your local changes to the following files would be overwritten by checkout:\n\tfile.txt",
        )
        .into();
        assert_eq!(
            err.suggestion.as_ref().unwrap().action_id,
            "stash-and-checkout"
        );
    }

    #[test]
    fn maps_no_upstream_to_publish_suggestion() {
        let err: AppError =
            git_error("fatal: The current branch feature has no upstream branch.").into();
        assert_eq!(err.suggestion.as_ref().unwrap().action_id, "publish");
    }

    #[test]
    fn maps_unrelated_histories_to_allow_unrelated_suggestion() {
        let err: AppError = git_error("fatal: refusing to merge unrelated histories").into();
        assert_eq!(
            err.suggestion.as_ref().unwrap().action_id,
            "allow-unrelated"
        );
    }

    #[test]
    fn unknown_errors_have_no_suggestion() {
        let err: AppError = git_error("fatal: some brand new error we've never seen").into();
        assert!(err.suggestion.is_none());
    }

    #[test]
    fn scrubs_userinfo_from_https_url_in_raw() {
        let err: AppError = git_error(
            "fatal: unable to access 'https://alice:hunter2@github.com/x/y.git/': Could not resolve host",
        )
        .into();
        assert!(!err.raw.contains("hunter2"));
        assert!(!err.raw.contains("alice"));
        assert!(err.raw.contains("***@github.com"));
    }

    #[test]
    fn scrub_leaves_urls_without_userinfo_untouched() {
        assert_eq!(
            scrub("fatal: unable to access 'https://github.com/x/y.git/'"),
            "fatal: unable to access 'https://github.com/x/y.git/'"
        );
    }
}
