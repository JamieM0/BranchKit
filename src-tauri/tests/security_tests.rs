//! CI security audit tests — ARCHITECTURE.md §8: "audit: grep -ri password\|token over logs in CI
//! test." BranchKit has no persistent log file to grep (no `log`/`tracing` sink is wired up), so
//! the equivalent, CI-safe-on-every-OS check is source-level: no file other than `credentials.rs`
//! (whose `get` branch legitimately writes `password=`/`username=` to stdout — that's git's own
//! credential-helper protocol, not a log) may contain a print/log statement that mentions a
//! password, secret, API key, or token. This catches an accidental `println!("token: {token}")`
//! debug line before it ships, without needing a real OS keychain in the runner.

use std::path::{Path, PathBuf};

use branchkit_lib::git::exec::{git_with_stdin, GitOpts};

#[path = "common/mod.rs"]
mod common;
use common::TestRepo;

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_source_file_logs_a_raw_secret_outside_the_credential_helper_protocol() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs_files(&src_dir, &mut files);
    assert!(!files.is_empty(), "expected to find source files under src/");

    let mut offenders = Vec::new();
    for file in files {
        // The credential-helper `get` handler in credentials.rs prints `username=`/`password=` to
        // stdout — that IS the git protocol response, not a log line, so it's the one allowed spot.
        if file.file_name().is_some_and(|n| n == "credentials.rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            let is_log_call = ["println!", "eprintln!", "dbg!", "log::", "tracing::"]
                .iter()
                .any(|marker| lower.contains(marker));
            let mentions_secret = ["password", "api_key", "apikey", "access_token", "\"token", " token"]
                .iter()
                .any(|marker| lower.contains(marker));
            if is_log_call && mentions_secret {
                offenders.push(format!("{}:{}: {}", file.display(), i + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "found logging statements that may leak a secret:\n{}",
        offenders.join("\n")
    );
}

/// ARCHITECTURE.md §8: every network command gets `-c credential.helper=` (clears any configured
/// helper) before pointing git at our own — verified here against a repo-level helper standing in
/// for "a system credential helper interfering with a plain HTTPS push" (DESIGN_SPEC's prompt
/// verification step). If the override didn't clear it first, git would run both helpers.
#[tokio::test]
async fn credential_helper_args_clear_a_previously_configured_helper() {
    let repo = TestRepo::init().await;
    let marker = repo.path().join("system-helper-ran");
    let marker_str = marker.to_string_lossy().to_string();
    repo.run(&[
        "config",
        "credential.helper",
        &format!("!touch '{marker_str}'; true"),
    ])
    .await;

    let helper = branchkit_lib::credentials::helper_config_args();
    let mut args: Vec<&str> = helper.iter().map(String::as_str).collect();
    args.extend(["credential", "fill"]);

    let _ = git_with_stdin(
        repo.path(),
        &args,
        GitOpts::default(),
        b"protocol=https\nhost=example.com\n\n",
    )
    .await;

    assert!(
        !marker.exists(),
        "the repo-configured credential helper ran despite the -c override that should clear it first"
    );
}
