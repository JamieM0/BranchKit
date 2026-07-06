//! GitHub REST calls — ARCHITECTURE.md §11. Originally six endpoints, plain `reqwest` + serde
//! structs (no octocrab, per ARCHITECTURE's "fewer deps, we need 6 endpoints"): current user, list
//! PRs, check-runs + combined status, create PR, merge PR; PR-head checkout is a git fetch, not a
//! REST call, and lives further down since it still needs the repo's op queue.
//!
//! SPEC-DEVIATION (ARCHITECTURE.md §11 / DESIGN_SPEC.md §12 "v1 scope"): two more endpoints below
//! — `list_orgs` and `create_repo` — back "publish an unpublished repo straight to a new GitHub
//! repo" (Jamie's request). Neither doc mentions repo creation; this is a deliberate scope add,
//! not an oversight. Same auth/rate-limit rules as the rest of this file apply.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::credentials;
use crate::error::AppError;
use crate::events::{ChangeKind, WatchedKind};
use crate::git::exec::{git, git_with_progress, GitOpts};
use crate::git::ops::{emit_changes, require_repo};
use crate::state::AppState;

use super::GithubUser;

const API_BASE: &str = "https://api.github.com";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("BranchKit")
        .build()
        .expect("static reqwest client config is valid")
}

fn auth_token() -> Result<zeroize::Zeroizing<String>, AppError> {
    credentials::get_secret(credentials::GITHUB_ACCOUNT).ok_or_else(|| {
        AppError::new(
            "Sign in to GitHub first (Settings → Integrations)",
            "no github token in keychain",
        )
    })
}

fn authed(request: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    request
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
}

pub(super) async fn fetch_current_user(token: &str) -> Result<GithubUser, AppError> {
    let resp: serde_json::Value = authed(client().get(format!("{API_BASE}/user")), token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| AppError::new("Could not read your GitHub profile", e.to_string()))?;
    Ok(GithubUser {
        login: resp["login"].as_str().unwrap_or_default().to_string(),
        avatar_url: resp["avatar_url"].as_str().unwrap_or_default().to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GithubOrg {
    pub login: String,
    pub avatar_url: String,
}

/// Orgs the signed-in user belongs to — the "create repo" dialog's owner picker (personal account
/// vs. an org), SPEC-DEVIATION per this file's header. `GET /user/orgs` covers the common case;
/// orgs a user belongs to but that hide membership from this endpoint just won't show up as a
/// choice, which is an acceptable v1 gap rather than a bug.
#[tauri::command]
pub async fn list_orgs() -> Result<Vec<GithubOrg>, AppError> {
    let token = auth_token()?;
    let resp: Vec<serde_json::Value> = authed(client().get(format!("{API_BASE}/user/orgs")), &token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .map_err(|e| AppError::new("Could not read your GitHub organizations", e.to_string()))?;

    Ok(resp
        .into_iter()
        .map(|o| GithubOrg {
            login: o["login"].as_str().unwrap_or_default().to_string(),
            avatar_url: o["avatar_url"].as_str().unwrap_or_default().to_string(),
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreatedGithubRepo {
    pub html_url: String,
    /// HTTPS clone URL — `credentials.rs` already special-cases `github.com` to answer the git
    /// credential helper with the signed-in OAuth token (see `github.com` branch there), so this
    /// works as `origin` immediately without any extra credential setup.
    pub clone_url: String,
}

/// Creates a new GitHub repository and returns its clone URL — the "publish an unpublished repo"
/// flow's first step (SPEC-DEVIATION per this file's header). `owner` is `None` for the signed-in
/// user's personal account, or an org login from `list_orgs`. Caller is responsible for then
/// `git remote add origin <cloneUrl>` and pushing — this command only talks to GitHub.
#[tauri::command]
pub async fn create_repo(
    owner: Option<String>,
    name: String,
    private: bool,
) -> Result<CreatedGithubRepo, AppError> {
    let token = auth_token()?;
    let url = match &owner {
        Some(org) if !org.is_empty() => format!("{API_BASE}/orgs/{org}/repos"),
        _ => format!("{API_BASE}/user/repos"),
    };

    let resp: serde_json::Value = authed(client().post(url), &token)
        .json(&serde_json::json!({ "name": name, "private": private }))
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AppError::new("GitHub couldn't create that repository", e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::new("Could not read GitHub's response", e.to_string()))?;

    Ok(CreatedGithubRepo {
        html_url: resp["html_url"].as_str().unwrap_or_default().to_string(),
        clone_url: resp["clone_url"].as_str().unwrap_or_default().to_string(),
    })
}

/// Parses `owner/repo` out of a `git remote get-url` value — both SSH (`git@github.com:o/r.git`)
/// and HTTPS (`https://github.com/o/r.git` / `https://github.com/o/r`) forms. Non-GitHub remotes
/// return `None`, which is exactly the "integration surfaces simply don't render" signal
/// (ARCHITECTURE.md §11).
pub fn parse_github_repo(remote_url: &str) -> Option<(String, String)> {
    let rest = remote_url
        .strip_prefix("git@github.com:")
        .or_else(|| remote_url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| remote_url.strip_prefix("https://github.com/"))
        .or_else(|| remote_url.strip_prefix("http://github.com/"))?;
    let trimmed = rest.trim_end_matches('/').trim_end_matches(".git");
    let (owner, repo) = trimmed.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

async fn owner_repo(handle: &crate::state::RepoHandle) -> Result<(String, String), AppError> {
    let output = git(&handle.path, &["remote", "get-url", "origin"], GitOpts::default()).await?;
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_github_repo(&url).ok_or_else(|| {
        AppError::new(
            "This repository's origin isn't on GitHub",
            format!("unrecognized remote url: {url}"),
        )
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub draft: bool,
    pub head_ref: String,
    pub base_ref: String,
    pub head_sha: String,
    pub author_login: String,
    pub author_avatar_url: String,
    pub html_url: String,
    pub comment_count: u32,
    /// Requested reviewers' logins — enough for the PR side panel's reviewer list without a
    /// second endpoint.
    pub reviewers: Vec<String>,
}

/// Open PRs for this repo's origin — the LEFT panel's PULL REQUESTS section (DESIGN_SPEC.md §5).
#[tauri::command]
pub async fn list_pull_requests(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<PullRequest>, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let token = auth_token()?;
    let (owner, repo) = owner_repo(&handle).await?;

    let resp: Vec<serde_json::Value> = authed(
        client().get(format!("{API_BASE}/repos/{owner}/{repo}/pulls?state=open&per_page=50")),
        &token,
    )
    .send()
    .await?
    .error_for_status()?
    .json()
    .await
    .map_err(|e| AppError::new("Could not read pull requests", e.to_string()))?;

    Ok(resp
        .into_iter()
        .map(|pr| PullRequest {
            number: pr["number"].as_u64().unwrap_or_default(),
            title: pr["title"].as_str().unwrap_or_default().to_string(),
            body: pr["body"].as_str().unwrap_or_default().to_string(),
            state: pr["state"].as_str().unwrap_or_default().to_string(),
            draft: pr["draft"].as_bool().unwrap_or(false),
            head_ref: pr["head"]["ref"].as_str().unwrap_or_default().to_string(),
            base_ref: pr["base"]["ref"].as_str().unwrap_or_default().to_string(),
            head_sha: pr["head"]["sha"].as_str().unwrap_or_default().to_string(),
            author_login: pr["user"]["login"].as_str().unwrap_or_default().to_string(),
            author_avatar_url: pr["user"]["avatar_url"].as_str().unwrap_or_default().to_string(),
            html_url: pr["html_url"].as_str().unwrap_or_default().to_string(),
            comment_count: pr["comments"].as_u64().unwrap_or_default() as u32,
            reviewers: pr["requested_reviewers"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|r| r["login"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckRun {
    pub name: String,
    /// "queued" | "in_progress" | "completed".
    pub status: String,
    /// "success" | "failure" | "neutral" | "cancelled" | "skipped" | "timed_out" | "action_required" | null while not completed.
    pub conclusion: Option<String>,
    pub html_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommitCheckStatus {
    /// "success" | "failure" | "pending" | "none" — the CI dot's color (DESIGN_SPEC.md §12).
    pub summary: String,
    pub runs: Vec<CheckRun>,
}

fn summarize(runs: &[CheckRun]) -> String {
    if runs.is_empty() {
        return "none".to_string();
    }
    if runs.iter().any(|r| r.status != "completed") {
        return "pending".to_string();
    }
    let failed = runs.iter().any(|r| {
        matches!(
            r.conclusion.as_deref(),
            Some("failure") | Some("timed_out") | Some("action_required") | Some("cancelled")
        )
    });
    if failed {
        "failure".to_string()
    } else {
        "success".to_string()
    }
}

/// CI dot + checks popover data for one commit sha (DESIGN_SPEC.md §12/§15.23). The frontend is
/// responsible for only calling this for visible rows, caching 60s, and never polling more than
/// once a minute per repo (ARCHITECTURE.md §11's rate-limit rule) — this command just answers.
#[tauri::command]
pub async fn get_check_status(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<CommitCheckStatus, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let token = auth_token()?;
    let (owner, repo) = owner_repo(&handle).await?;

    let resp: serde_json::Value = authed(
        client().get(format!(
            "{API_BASE}/repos/{owner}/{repo}/commits/{sha}/check-runs"
        )),
        &token,
    )
    .send()
    .await?
    .error_for_status()?
    .json()
    .await
    .map_err(|e| AppError::new("Could not read CI status", e.to_string()))?;

    let runs: Vec<CheckRun> = resp["check_runs"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|r| CheckRun {
                    name: r["name"].as_str().unwrap_or_default().to_string(),
                    status: r["status"].as_str().unwrap_or_default().to_string(),
                    conclusion: r["conclusion"].as_str().map(str::to_string),
                    html_url: r["html_url"].as_str().unwrap_or_default().to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(CommitCheckStatus {
        summary: summarize(&runs),
        runs,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CreatedPr {
    pub number: u64,
    pub html_url: String,
}

/// The Create-PR panel's [Create] (DESIGN_SPEC.md §12). Never opens the browser itself — the
/// caller's toast offers that as its one action verb (§8).
#[tauri::command]
pub async fn create_pull_request(
    state: State<'_, AppState>,
    repo_id: String,
    base: String,
    head: String,
    title: String,
    body: String,
) -> Result<CreatedPr, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let token = auth_token()?;
    let (owner, repo) = owner_repo(&handle).await?;

    let resp: serde_json::Value = authed(
        client().post(format!("{API_BASE}/repos/{owner}/{repo}/pulls")),
        &token,
    )
    .json(&serde_json::json!({ "title": title, "head": head, "base": base, "body": body }))
    .send()
    .await?
    .error_for_status()
    .map_err(|e| AppError::new("GitHub rejected the pull request", e.to_string()))?
    .json()
    .await
    .map_err(|e| AppError::new("Could not read GitHub's response", e.to_string()))?;

    Ok(CreatedPr {
        number: resp["number"].as_u64().unwrap_or_default(),
        html_url: resp["html_url"].as_str().unwrap_or_default().to_string(),
    })
}

/// The PR side panel's **Merge…** (DESIGN_SPEC.md §12). `method` is "merge" | "squash" | "rebase".
#[tauri::command]
pub async fn merge_pull_request(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    number: u64,
    method: String,
) -> Result<(), AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let token = auth_token()?;
    let (owner, repo) = owner_repo(&handle).await?;

    authed(
        client().put(format!(
            "{API_BASE}/repos/{owner}/{repo}/pulls/{number}/merge"
        )),
        &token,
    )
    .json(&serde_json::json!({ "merge_method": method }))
    .send()
    .await?
    .error_for_status()
    .map_err(|e| AppError::new("GitHub couldn't merge this pull request", e.to_string()))?;

    // The merge landed on GitHub, not locally — a Refs/Remote refresh lets a subsequent fetch/pull
    // surface it the normal way rather than us guessing at local ref state here.
    emit_changes(&app, &repo_id, &[ChangeKind::Remote]);
    Ok(())
}

/// Fetches a PR's head (works for fork PRs too, via `pull/<n>/head`) and checks it out as
/// `pr-<n>` — the PR side panel's **Checkout branch** (DESIGN_SPEC.md §12, ARCHITECTURE.md §11).
/// Runs through the repo op queue and the credential-helper injection like any other network op.
#[tauri::command]
pub async fn checkout_pr_head(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    number: u64,
) -> Result<String, AppError> {
    let handle = require_repo(&state, &repo_id)?;
    let _guard = handle.op_queue.lock().await;
    let local_branch = format!("pr-{number}");
    let refspec = format!("pull/{number}/head:{local_branch}");

    handle.begin_self_op(&[WatchedKind::Refs, WatchedKind::Remote]);
    let mut helper_args = credentials::helper_config_args();
    let helper_refs: Vec<&str> = helper_args.iter().map(String::as_str).collect();
    let mut fetch_args: Vec<&str> = helper_refs.clone();
    fetch_args.extend(["fetch", "origin", &refspec, "--progress"]);
    let fetch_result = git_with_progress(&handle.path, &fetch_args, GitOpts::network(), |_| {}).await;
    helper_args.clear();
    if let Err(e) = fetch_result {
        return Err(e.into());
    }

    handle.begin_self_op(&[WatchedKind::Head, WatchedKind::WorkingTree, WatchedKind::Index]);
    // The local branch may already exist from a previous checkout of the same PR — force-update
    // it to the freshly fetched head rather than erroring.
    let checkout = git(
        &handle.path,
        &["checkout", "-B", &local_branch, "FETCH_HEAD"],
        GitOpts::default(),
    )
    .await;

    emit_changes(&app, &repo_id, &[ChangeKind::Head, ChangeKind::Refs]);
    checkout?;
    Ok(local_branch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_remote() {
        assert_eq!(
            parse_github_repo("git@github.com:owner/repo.git"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn parses_https_remote() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo.git"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn parses_https_remote_without_dot_git_suffix() {
        assert_eq!(
            parse_github_repo("https://github.com/owner/repo"),
            Some(("owner".to_string(), "repo".to_string()))
        );
    }

    #[test]
    fn non_github_remote_is_none() {
        assert_eq!(parse_github_repo("https://gitlab.com/owner/repo.git"), None);
    }

    #[test]
    fn summarize_pending_when_any_run_incomplete() {
        let runs = vec![CheckRun {
            name: "build".into(),
            status: "in_progress".into(),
            conclusion: None,
            html_url: String::new(),
        }];
        assert_eq!(summarize(&runs), "pending");
    }

    #[test]
    fn summarize_failure_when_any_completed_run_failed() {
        let runs = vec![
            CheckRun {
                name: "build".into(),
                status: "completed".into(),
                conclusion: Some("success".into()),
                html_url: String::new(),
            },
            CheckRun {
                name: "test".into(),
                status: "completed".into(),
                conclusion: Some("failure".into()),
                html_url: String::new(),
            },
        ];
        assert_eq!(summarize(&runs), "failure");
    }

    #[test]
    fn summarize_none_when_no_runs() {
        assert_eq!(summarize(&[]), "none");
    }
}
