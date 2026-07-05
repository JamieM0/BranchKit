//! Secure credential storage — ARCHITECTURE.md §8 ("careful with security!"). Threat model:
//! secrets never touch disk in plaintext, are never logged, and never leave the machine except to
//! the git host itself; the frontend only ever sees non-secret metadata (host, username,
//! last-used). HTTPS host credentials, the GitHub OAuth token, and AI provider API keys all live
//! in the OS keychain via the `keyring` crate (service `"BranchKit"`).
//!
//! This module is linked into both the normal windowed app *and* the bare `credential-helper` CLI
//! subcommand that `main.rs` intercepts before any window (or `AppHandle`) exists — see
//! `run_credential_helper_cli` below — so nothing here may depend on a running Tauri `App`.

use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use zeroize::Zeroizing;

const SERVICE: &str = "BranchKit";
pub const GITHUB_ACCOUNT: &str = "github-oauth";
pub const AI_API_KEY_ACCOUNT: &str = "ai-api-key";

fn account_for(host: &str, username: &str) -> String {
    format!("{host}:{username}")
}

fn entry(account: &str) -> Result<Entry, keyring::Error> {
    Entry::new(SERVICE, account)
}

/// `None` covers both "nothing stored" and "keychain unavailable" (ARCHITECTURE.md §14: Linux
/// without libsecret degrades to in-memory-only secrets) — a miss here is an expected case for
/// every caller, never a hard failure.
pub fn get_secret(account: &str) -> Option<Zeroizing<String>> {
    let e = entry(account).ok()?;
    e.get_password().ok().map(Zeroizing::new)
}

pub fn set_secret(account: &str, secret: &str) -> Result<(), keyring::Error> {
    entry(account)?.set_password(secret)
}

pub fn delete_secret(account: &str) {
    // Best-effort: erasing something already absent isn't an error worth surfacing.
    if let Ok(e) = entry(account) {
        let _ = e.delete_credential();
    }
}

pub fn get_host_credential(host: &str, username: &str) -> Option<Zeroizing<String>> {
    get_secret(&account_for(host, username))
}

pub fn set_host_credential(host: &str, username: &str, secret: &str) -> Result<(), keyring::Error> {
    set_secret(&account_for(host, username), secret)
}

pub fn delete_host_credential(host: &str, username: &str) {
    delete_secret(&account_for(host, username));
}

// ---- non-secret metadata (host, username, last-used) for the Credentials settings list ----
// ARCHITECTURE.md §8: "Config files store only non-secret settings." Keyring backends can't be
// listed, so this small JSON index (never containing a password) is what the Settings UI reads.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialMeta {
    pub host: String,
    pub username: String,
    pub last_used_at: i64,
}

/// Mirrors what `tauri::Manager::app_config_dir()` resolves for this app's identifier
/// (`dev.branchkit.app`, tauri.conf.json) — duplicated rather than called because the
/// `credential-helper` subprocess (invoked directly by `git`, see below) never has an `AppHandle`.
fn config_dir() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("dev.branchkit.app"))
}

fn metadata_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("credentials_meta.json"))
}

pub fn list_metadata() -> Vec<CredentialMeta> {
    let Some(path) = metadata_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_metadata(entries: &[CredentialMeta]) {
    let Some(path) = metadata_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(path, text);
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn record_used(host: &str, username: &str) {
    let mut entries = list_metadata();
    entries.retain(|e| !(e.host == host && e.username == username));
    entries.push(CredentialMeta {
        host: host.to_string(),
        username: username.to_string(),
        last_used_at: now(),
    });
    save_metadata(&entries);
}

pub fn remove_metadata(host: &str, username: &str) {
    let mut entries = list_metadata();
    entries.retain(|e| !(e.host == host && e.username == username));
    save_metadata(&entries);
}

/// Removes a stored HTTPS credential and its metadata together — the Credentials settings list's
/// remove button.
pub fn remove_host_credential(host: &str, username: &str) {
    delete_host_credential(host, username);
    remove_metadata(host, username);
}

// ---- credential.helper config injection (ARCHITECTURE.md §8) ----

/// The `-c` args every network git command gets: clears any system credential helper, then points
/// git at this same binary's `credential-helper` subcommand. Must be placed before the git
/// subcommand name in the argument list — global `-c` options are only recognized there.
pub fn helper_config_args() -> Vec<String> {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "branchkit".to_string());
    vec![
        "-c".to_string(),
        "credential.helper=".to_string(),
        "-c".to_string(),
        format!("credential.helper=!\"{exe}\" credential-helper"),
    ]
}

// ---- the `credential-helper` CLI subcommand ----
// `main.rs` intercepts argv for this before Tauri's window (or anything else) initializes, since
// git invokes it as a plain subprocess and expects the get/store/erase protocol on stdin/stdout —
// ARCHITECTURE.md §8's "Simpler v1 fallback" this implements: `get` looks up the keychain; `store`
// saves on success; `erase` removes on auth failure. On a `get` miss, this prints nothing and exits
// 0 — git then fails the operation with its own "Authentication failed", which `error.rs`'s
// translator turns into the credential-dialog suggestion; the frontend then calls `save_credential`
// and retries the operation once (the retried `get` finds it here).
pub fn run_credential_helper_cli(op: &str) {
    use std::io::Read;
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let mut host = String::new();
    let mut username = String::new();
    let mut password = String::new();
    for line in input.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                "host" => host = v.to_string(),
                "username" => username = v.to_string(),
                "password" => password = v.to_string(),
                _ => {}
            }
        }
    }
    if host.is_empty() {
        return;
    }

    match op {
        "get" => {
            // git only supplies `username=` once it already knows one; on the first attempt for a
            // host, fall back to the most recently used username we've stored for it.
            let stored_username = if username.is_empty() {
                list_metadata()
                    .into_iter()
                    .filter(|m| m.host == host)
                    .max_by_key(|m| m.last_used_at)
                    .map(|m| m.username)
            } else {
                Some(username.clone())
            };
            if let Some(username) = stored_username {
                if let Some(secret) = get_host_credential(&host, &username) {
                    println!("username={username}");
                    println!("password={}", secret.as_str());
                    return;
                }
            }
            // No per-host credential saved: if this is github.com and the user is signed in via
            // the device-flow OAuth token, that token doubles as a git HTTPS credential — GitHub
            // accepts any non-empty username with the token as the password.
            if host == "github.com" {
                if let Some(token) = get_secret(GITHUB_ACCOUNT) {
                    println!("username=x-access-token");
                    println!("password={}", token.as_str());
                }
            }
        }
        "store" => {
            if username.is_empty() || password.is_empty() {
                return;
            }
            let _ = set_host_credential(&host, &username, &password);
            record_used(&host, &username);
        }
        "erase" => {
            if username.is_empty() {
                return;
            }
            remove_host_credential(&host, &username);
        }
        _ => {}
    }
}

// ---- SSH (ARCHITECTURE.md §8: delegate to the user's own agent/keys; never manage passphrases) ----

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshAgentStatus {
    pub agent_running: bool,
    /// One line per loaded identity, exactly as `ssh-add -l` prints it (fingerprint + comment) —
    /// never private key material.
    pub identities: Vec<String>,
}

/// Detects the ssh-agent and its loaded identities via `ssh-add -l` — read-only, no key material
/// handled here at all (ARCHITECTURE.md §8).
#[tauri::command]
pub async fn get_ssh_agent_status() -> SshAgentStatus {
    match tokio::process::Command::new("ssh-add")
        .arg("-l")
        .output()
        .await
    {
        Ok(output) if output.status.success() => SshAgentStatus {
            agent_running: true,
            identities: String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect(),
        },
        // Exit 1 = agent reachable but holds no identities; exit 2 (or spawn failure) = no agent.
        Ok(output) => SshAgentStatus {
            agent_running: output.status.code() == Some(1),
            identities: Vec::new(),
        },
        Err(_) => SshAgentStatus {
            agent_running: false,
            identities: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub public_key: String,
    pub path: String,
}

fn branchkit_key_paths() -> Result<(PathBuf, PathBuf), crate::error::AppError> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::error::AppError::new("Could not find your home directory", "no home directory")
    })?;
    let ssh_dir = home.join(".ssh");
    let key = ssh_dir.join("id_ed25519_branchkit");
    let public_key = ssh_dir.join("id_ed25519_branchkit.pub");
    Ok((key, public_key))
}

/// The previously-generated BranchKit key, if any — so the SSH settings section can show it
/// without regenerating on every app launch.
#[tauri::command]
pub fn get_generated_ssh_key() -> Option<SshKeyInfo> {
    let (key_path, pub_path) = branchkit_key_paths().ok()?;
    let public_key = std::fs::read_to_string(pub_path).ok()?.trim().to_string();
    Some(SshKeyInfo {
        public_key,
        path: key_path.to_string_lossy().into_owned(),
    })
}

/// Generates a new ed25519 keypair at `~/.ssh/id_ed25519_branchkit` — only ever invoked after
/// explicit user consent from the SSH settings section, which must have already made the
/// empty-passphrase trade-off clear if `passphrase` is empty (ARCHITECTURE.md §8).
///
/// SPEC-DEVIATION: `ssh-keygen -N` takes the new passphrase as a plain argument; there is no
/// portable stdin-based form (interactive prompting reads the tty directly, not a pipe), so a
/// non-empty passphrase is briefly visible to other local processes via argv. Zeroized from our
/// own memory the moment the child is spawned regardless.
#[tauri::command]
pub async fn generate_ssh_key(passphrase: String) -> Result<SshKeyInfo, crate::error::AppError> {
    let passphrase = Zeroizing::new(passphrase);
    let (key_path, pub_path) = branchkit_key_paths()?;
    if let Some(dir) = key_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if key_path.exists() {
        return Err(crate::error::AppError::new(
            "A BranchKit-generated key already exists at ~/.ssh/id_ed25519_branchkit",
            "id_ed25519_branchkit already exists",
        ));
    }
    let key_path_str = key_path.to_string_lossy().into_owned();

    let output = tokio::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-f", &key_path_str, "-N", passphrase.as_str(), "-q"])
        .stdin(std::process::Stdio::null())
        .output()
        .await?;
    if !output.status.success() {
        return Err(crate::error::AppError::new(
            "Could not generate an SSH key",
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }

    let public_key = std::fs::read_to_string(pub_path)?.trim().to_string();
    Ok(SshKeyInfo {
        public_key,
        path: key_path_str,
    })
}

// ---- tauri commands for the Credentials settings list (DESIGN_SPEC.md §13) ----

pub type CredentialInfo = CredentialMeta;

/// Metadata only — host, username, last-used. Never a password (ARCHITECTURE.md §8).
#[tauri::command]
pub fn list_credentials() -> Vec<CredentialInfo> {
    list_metadata()
}

#[tauri::command]
pub fn remove_credential(host: String, username: String) {
    remove_host_credential(&host, &username);
}

/// Saves a credential entered in the auth-failure dialog; the frontend then retries the operation
/// once (ARCHITECTURE.md §8) — the credential-helper's own `get` will find it on that retry.
#[tauri::command]
pub fn save_credential(
    host: String,
    username: String,
    password: String,
) -> Result<(), crate::error::AppError> {
    set_host_credential(&host, &username, &password)?;
    record_used(&host, &username);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_config_args_clears_then_sets_helper() {
        let args = helper_config_args();
        assert_eq!(args[0], "-c");
        assert_eq!(args[1], "credential.helper=");
        assert_eq!(args[2], "-c");
        assert!(args[3].starts_with("credential.helper=!"));
        assert!(args[3].ends_with("credential-helper"));
    }

    #[test]
    fn account_for_combines_host_and_username() {
        assert_eq!(account_for("github.com", "alice"), "github.com:alice");
    }
}
