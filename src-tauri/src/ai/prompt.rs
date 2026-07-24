//! The commit-message prompt — ARCHITECTURE.md §10's exact template and truncation rules. Pure,
//! provider-agnostic: builds the `{system, user}` messages from an already-fetched diff stat/text,
//! and parses a model's raw reply back into `{summary, description}`. Kept dependency-free of any
//! HTTP client so it's trivially unit-testable.

use serde::{Deserialize, Serialize};

use crate::settings::CommitStyle;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCommitMessage {
    pub summary: String,
    pub description: String,
}

/// The complete, Markdown-formatted explanation returned for an existing commit. Unlike commit
/// messages, explanations deliberately do not have a style setting: they are connected prose,
/// never Conventional Commits shorthand.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCommitExplanation {
    pub markdown: String,
}

/// Splits a multi-file unified diff into one string per file, each starting at its
/// `diff --git a/… b/…` header — the unit the per-file 150-line cap (ARCHITECTURE.md §10) applies to.
fn split_into_files(diff: &str) -> Vec<String> {
    let mut files = Vec::new();
    let mut current = String::new();
    for line in diff.lines() {
        if line.starts_with("diff --git") && !current.is_empty() {
            files.push(std::mem::take(&mut current));
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.is_empty() {
        files.push(current);
    }
    files
}

/// Caps one file's diff text to `max_lines`, noting how many were dropped.
fn cap_file_lines(file: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = file.lines().collect();
    if lines.len() <= max_lines {
        return file.to_string();
    }
    let mut out = lines[..max_lines].join("\n");
    out.push_str(&format!("\n… ({} more lines)\n", lines.len() - max_lines));
    out
}

/// Applies both truncation rules (ARCHITECTURE.md §10): per-file cap of 150 lines, then a total
/// character cap (`max_chars`) enforced at file granularity — once adding a whole (already
/// per-file-capped) file would exceed the budget, stop and note how many files were left out.
/// The very first file is always included in full (capped only per-line) even if it alone exceeds
/// the budget, so a single huge file never produces an empty prompt.
pub fn truncate_diff(diff: &str, max_chars: usize) -> String {
    let files = split_into_files(diff);
    let mut out = String::new();
    for (i, file) in files.iter().enumerate() {
        let capped = cap_file_lines(file, 150);
        if i > 0 && out.len() + capped.len() > max_chars {
            let remaining = files.len() - i;
            out.push_str(&format!(
                "\n… ({remaining} more file{})\n",
                if remaining == 1 { "" } else { "s" }
            ));
            break;
        }
        out.push_str(&capped);
    }
    out
}

/// Builds the `{system, user}` message pair sent to whichever provider is configured
/// (ARCHITECTURE.md §10). `stat` is `git diff --cached --stat` (or `git diff --stat` for the
/// unstaged fallback); `diff` is the corresponding full diff, truncated here.
pub fn build_commit_prompt(
    style: CommitStyle,
    stat: &str,
    diff: &str,
    max_diff_size_kb: u32,
) -> Vec<ChatMessage> {
    let mut system =
        String::from("Write a git commit message. First line \u{2264}72 chars, imperative mood");
    if style == CommitStyle::Conventional {
        system.push_str(", Conventional Commits format");
    }
    system.push_str(
        ". Then a blank line and 1\u{2013}4 bullet body only if the change is non-trivial. \
         Describe the change in your own words — never quote, repeat, or paraphrase the diff \
         or file list back verbatim. Output raw text only: no markdown, no code fences, no \
         headings, nothing before or after the message itself.",
    );
    if style == CommitStyle::Conventional {
        // Word budgets specific to Conventional Commits (requested separately from the general
        // length rules above): the subject's descriptive words — i.e. everything after the
        // `type(scope):` prefix, which doesn't count against this budget — are capped to 5
        // words max, and each body bullet (one per changed file) is capped to 5–15 words.
        system.push_str(
            " In the subject line, the descriptive words after the `type(scope):` prefix \
             (the prefix itself doesn't count) must be a maximum of 5 words. Each body bullet \
             covers one changed file in 5\u{2013}15 words.",
        );
    }

    let max_chars = (max_diff_size_kb as usize).saturating_mul(1024);
    let truncated = truncate_diff(diff, max_chars);
    let user = format!("{}\n\n{}", stat.trim_end(), truncated);

    vec![ChatMessage::system(system), ChatMessage::user(user)]
}

/// Builds the one request used to explain an existing commit. `commit_show` is intentionally not
/// truncated: an explanation that omits part of a commit is worse than no explanation at all.
/// This also intentionally ignores `AiSettings::style` and `max_diff_size_kb`; those settings
/// are exclusively for authoring new commit messages.
pub fn build_commit_explanation_prompt(commit_show: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(
            "Explain this entire git commit for a developer. Cover every changed file and every \
             meaningful behavioral, data-flow, API, test, configuration, and migration implication. \
             Use compact connected prose, grouped with useful Markdown headings only when they \
             improve scanning. Be terse but complete; do not use Conventional Commits syntax, \
             shortform, or a file-by-file diff recital. Mention uncertainty only when the patch \
             cannot establish intent. Output Markdown only.",
        ),
        ChatMessage::user(format!("Full commit (metadata and complete patch):\n\n{commit_show}")),
    ]
}

/// Strips a single Markdown code fence wrapping the *entire* reply (```` ``` ```` or ```` ```text ````
/// on their own first/last line) — small/local models routinely ignore "no markdown" instructions
/// and fence their answer anyway. Only strips a fence that wraps the whole text, never one that
/// merely appears inside a body bullet.
fn strip_wrapping_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(after_open) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    // Skip an optional language tag on the opening fence's own line (e.g. `text`, `markdown`).
    let after_open = match after_open.find('\n') {
        Some(nl) => &after_open[nl + 1..],
        None => after_open,
    };
    match after_open.trim_end().strip_suffix("```") {
        Some(body) => body.trim(),
        None => trimmed,
    }
}

/// Parses a model's raw reply: first line → summary, the rest (minus the blank separator) →
/// description (ARCHITECTURE.md §10).
pub fn parse_commit_message(text: &str) -> GeneratedCommitMessage {
    let text = strip_wrapping_fence(text);
    let mut lines = text.lines();
    let summary = lines.next().unwrap_or("").trim().to_string();
    let rest = lines.collect::<Vec<_>>().join("\n");
    GeneratedCommitMessage {
        summary,
        description: rest.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_block(path: &str, lines: usize) -> String {
        let mut s = format!("diff --git a/{path} b/{path}\n--- a/{path}\n+++ b/{path}\n@@ -1,{lines} +1,{lines} @@\n");
        for i in 0..lines {
            s.push_str(&format!("+line {i}\n"));
        }
        s
    }

    #[test]
    fn caps_a_single_file_at_150_lines() {
        // 4 header lines + 300 `+line` lines = 304 total; capped to the first 150.
        let diff = file_block("a.txt", 300);
        let out = truncate_diff(&diff, 1_000_000);
        assert!(out.contains("(154 more lines)"));
        assert_eq!(out.lines().filter(|l| l.starts_with("+line")).count(), 146);
    }

    #[test]
    fn leaves_a_short_file_untouched() {
        let diff = file_block("a.txt", 10);
        let out = truncate_diff(&diff, 1_000_000);
        assert_eq!(out, diff);
    }

    #[test]
    fn notes_more_files_once_total_budget_is_exceeded() {
        let diff = format!(
            "{}{}{}",
            file_block("a.txt", 5),
            file_block("b.txt", 5),
            file_block("c.txt", 5)
        );
        // Budget fits only the first file's ~90 chars.
        let out = truncate_diff(&diff, 100);
        assert!(out.contains("diff --git a/a.txt"));
        assert!(!out.contains("b.txt"));
        assert!(out.contains("(2 more files)"));
    }

    #[test]
    fn always_includes_the_first_file_even_over_budget() {
        let diff = file_block("a.txt", 5);
        let out = truncate_diff(&diff, 1);
        assert!(out.contains("diff --git a/a.txt"));
    }

    #[test]
    fn conventional_style_mentions_conventional_commits_in_system_prompt() {
        let messages = build_commit_prompt(CommitStyle::Conventional, "1 file changed", "", 8);
        assert!(messages[0].content.contains("Conventional Commits"));
    }

    #[test]
    fn plain_style_omits_conventional_commits_mention() {
        let messages = build_commit_prompt(CommitStyle::Plain, "1 file changed", "", 8);
        assert!(!messages[0].content.contains("Conventional Commits"));
    }

    #[test]
    fn conventional_style_adds_the_word_budgets() {
        let messages = build_commit_prompt(CommitStyle::Conventional, "1 file changed", "", 8);
        assert!(messages[0].content.contains("maximum of 5 words"));
        assert!(messages[0].content.contains("5\u{2013}15 words"));
    }

    #[test]
    fn plain_style_omits_the_word_budgets() {
        let messages = build_commit_prompt(CommitStyle::Plain, "1 file changed", "", 8);
        assert!(!messages[0].content.contains("maximum of 5 words"));
        assert!(!messages[0].content.contains("5\u{2013}15 words"));
    }

    #[test]
    fn user_message_contains_stat_then_diff() {
        let messages = build_commit_prompt(CommitStyle::Plain, "STATLINE", "DIFFTEXT", 8);
        assert!(messages[1].content.starts_with("STATLINE"));
        assert!(messages[1].content.contains("DIFFTEXT"));
    }

    #[test]
    fn max_diff_size_kb_bounds_the_truncation_budget() {
        let diff = format!("{}{}", file_block("a.txt", 5), file_block("b.txt", 5));
        let messages = build_commit_prompt(CommitStyle::Plain, "stat", &diff, 0);
        // A ~0-char budget still keeps the first file in full.
        assert!(messages[1].content.contains("a.txt"));
        assert!(messages[1].content.contains("(1 more file)"));
    }

    #[test]
    fn parses_summary_and_description() {
        let msg = parse_commit_message("Fix the thing\n\n- did a\n- did b\n");
        assert_eq!(msg.summary, "Fix the thing");
        assert_eq!(msg.description, "- did a\n- did b");
    }

    #[test]
    fn strips_a_fence_with_a_language_tag_wrapping_the_whole_reply() {
        let msg = parse_commit_message("```text\nFix the thing\n\n- did a\n```");
        assert_eq!(msg.summary, "Fix the thing");
        assert_eq!(msg.description, "- did a");
    }

    #[test]
    fn strips_a_bare_fence_wrapping_the_whole_reply() {
        let msg = parse_commit_message("```\nFix the thing\n```");
        assert_eq!(msg.summary, "Fix the thing");
        assert_eq!(msg.description, "");
    }

    #[test]
    fn leaves_an_unfenced_reply_untouched() {
        let msg = parse_commit_message("Fix the thing\n\nbody");
        assert_eq!(msg.summary, "Fix the thing");
        assert_eq!(msg.description, "body");
    }

    #[test]
    fn parses_summary_only_reply() {
        let msg = parse_commit_message("Fix the thing");
        assert_eq!(msg.summary, "Fix the thing");
        assert_eq!(msg.description, "");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let msg = parse_commit_message("\n\n  Fix the thing  \n\nbody line\n\n");
        assert_eq!(msg.summary, "Fix the thing");
        assert_eq!(msg.description, "body line");
    }

    #[test]
    fn explanation_prompt_uses_the_full_commit_and_ignores_commit_message_style() {
        let complete_patch = "commit abc\n\ndiff --git a/a b/a\n+all changes";
        let messages = build_commit_explanation_prompt(complete_patch);
        assert!(messages[1].content.contains(complete_patch));
        assert!(messages[0].content.contains("entire git commit"));
        assert!(messages[0].content.contains("do not use Conventional Commits"));
    }
}
