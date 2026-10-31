# BranchKit user experience audit

Audit date: 31 October 2026. Audience: BranchKit's maintainer. Use this report to prioritize reliability work before adding features.

## Product intent and current behavior

BranchKit aims to make everyday Git work understandable and recoverable. Its main promise is a visual graph, accurate working directory status, safe staging, and conflict resolution through the Keep Panel.

The implementation covers much of that scope. Rust runs system Git commands, serializes mutations, parses status, and watches repository changes. Svelte presents graph navigation, staging, diffs, commits, stashes, worktrees, remote operations, GitHub integration, and optional AI assistance.

The largest gap is confidence at workflow boundaries. A parser can return correct data while the interface displays an older snapshot. Recovery and conflict handling also need stronger guarantees for unusual files.

This audit supersedes blanket pass claims in [the earlier QA report](docs/QA.md). That report remains a historical record.

## Verification scope

The audit combines source inspection, real Git fixtures, frontend regression tests, static checks, and native macOS workflow checks.

| Area | Evidence | Confidence |
| --- | --- | --- |
| Repository recognition | Real Git tests for nested folders, spaces, symlinks, missing paths, and ordinary folders | Verified at the backend |
| Fresh repository | New tests cover symbolic branch names, absent commit IDs, and nested untracked files | Verified at the backend |
| Working directory states | Fixtures cover clean, untracked, modified, deleted, staged, partially staged, renamed, and conflicted files | Verified at the backend |
| Branch state | Fixtures cover attached HEAD, detached HEAD, ahead/behind counts, annotated tags, and removed upstreams | Verified at the backend |
| Linked worktrees | Fixtures exercise parsers and shared Git directory resolution | Verified at the backend; native refresh checks recorded separately |
| Staging and recovery | Real Git tests cover partial staging and discard restoration, including binary bytes | Verified for covered fixtures |
| Conflict operations | Fixtures cover merge, rebase, cherry-pick, revert, and stash conflict detection | Verified for covered fixtures; limitations remain |
| GitHub and AI services | Source inspection and existing unit tests | Live account and provider workflows unverified |
| Windows and Linux | Source inspection and existing CI configuration | Native execution unverified in this session |
| Performance | Existing graph architecture and historical measurements | No new release frame trace or startup benchmark |

A passing fixture proves its scenario. It does not prove every operation, configuration, or filesystem combination.

## Changes from this audit

| Problem | User impact | Change |
| --- | --- | --- |
| Installed macOS executable was Intel-only | Apple Silicon required Rosetta; the installed app failed to launch here | Build a native arm64 app; configure future macOS releases as universal binaries |
| Graph refs required an existing HEAD commit | Fresh repositories opened but failed to load the graph | Preserve the initial branch name with an empty commit ID |
| Status collapsed new directories | New files appeared as a directory entry, breaking file-level inspection | Request every untracked file from Git |
| Status reads could complete out of order | Rapid switching could show another repository's files | Ignore superseded status requests and subscriptions |
| Graph and conflict subscriptions could survive tab switches | Older reads could overwrite current state | Guard subscriptions and asynchronous reads by session |
| Shared worktree refs were unwatched | External commits or fetches could leave linked worktrees stale | Watch the common Git directory as well as private worktree state |
| Ref changes updated labels without graph topology | External branch movement could leave commits missing | Reload topology when ref targets change |
| Event storms discarded Git event kinds | A checkout or build could leave branch or index state stale | Refresh all relevant state after an event storm |
| Tab branch labels stayed at their opening value | Checkout and detached state disagreed across the interface | Refresh tab labels after HEAD and ref events |
| Open diffs did not follow status updates | External edits could leave an old diff visible | Reload working copy diffs after status changes |
| Clean status hid the entire working directory panel | No clear confirmation that Git recognized a clean repository | Keep the panel visible with explicit clean status |
| Status failures lacked a visible recovery action | Empty or stale lists could look authoritative | Show the failure and a Retry status action |
| Binary discard backups lacked binary content | Undo could not restore a discarded binary file | Save binary patches and test exact restoration |
| Failed untracked backups could continue silently | Discard could delete a file without a backup | Fail the backup before discarding |
| Git availability check was not connected to startup | Missing or old Git surfaced as unrelated workflow failures | Block startup with an install link and Retry action |
| Identity lookup failures left onboarding waiting | First launch could remain on a spinner | Show an error and allow identity entry or Skip |
| Commit selection survived repository switches | Details could request a commit from the previous repository | Clear graph selection when switching repositories |

## Keychain password prompts

Startup reads the GitHub credential and AI API key. The installed app lacks a stable signing identity, so existing Keychain access grants can fail after rebuilds.

Credential reads, writes, deletions, and storage checks now disable password dialogs for the duration of each Keychain call. A serialized guard restores the previous process setting afterward.

The change preserves Keychain access controls. An inaccessible credential remains inaccessible; BranchKit shows a warning and supports reconnecting integrations for the session. It never unlocks Keychain or asks for the login password.

A regression test verifies disabled interaction during a call and restoration after failure. Local app bundles also receive a complete ad-hoc signature. A stable Developer ID signature still requires a configured signing identity.

See [Apple's Keychain interaction API](https://developer.apple.com/documentation/security/seckeychainsetuserinteractionallowed(_:)) and [Tauri's macOS signing instructions](https://v2.tauri.app/distribute/sign/macos/).

Session-only HTTPS credentials still need a bridge to the Git helper subprocess. The existing memory fallback does not cross process boundaries. SSH and persistent accessible credentials avoid this limitation.

## Remaining findings, in priority order

### High: conflict resolution does not preserve every file type

`git/conflict.rs` converts blob contents with `from_utf8_lossy` and splits them into text lines. Binary and invalid UTF-8 contents can change during reconstruction.

`keepPanel.svelte.ts` always joins nonempty results with LF and appends a final newline. CRLF files and files without a final newline therefore change beyond the chosen conflict lines.

`confirm_file` always writes a file and stages it. Choosing a deleted side can create an empty file instead of deleting the path. Symlink conflicts also require explicit handling.

These are correctness gaps in the flagship workflow. Before presenting the Keep Panel as universally safe, preserve file metadata and bytes or reject unsupported cases visibly. Add end-to-end fixtures that inspect the final Git tree, rather than only computed regions.

### High: conflict stages and multi-step operations need stronger validation

`read_stage` treats every Git failure as an absent stage. A timeout or read failure can therefore look like a legitimate deletion.

The session retains per-file reducers while an operation continues. A later rebase conflict on the same path can reuse choices from an earlier step. An external edit can also invalidate the loaded stage snapshot.

Identify each conflict step by its operation and stage object IDs. Recompute choices when those identities change. Confirm against the same snapshot that the user reviewed.

### High: discard recovery has limits beyond ordinary files

The binary patch omission is fixed. Other gaps remain:

- The recovery copy treats untracked symlinks as file contents, losing link identity.
- The size cap can evict an oversized new backup before the destructive operation finishes.
- Discard All assumes `HEAD` exists, so a fresh repository needs separate handling.
- Restoring combined changes recreates content but does not restore the original staged and unstaged split.
- A recovery patch can fail after later edits. The interface needs a clear explanation and a way to retain or export the recovery data.

The claim that every discard is recoverable for seven days needs qualification until these cases have tests and explicit behavior.

### Medium: external changes can fall inside self-event suppression

The watcher drops selected event kinds for 1.5 seconds after an app mutation begins. An unrelated external change during that interval can disappear with the app's own events.

Keep a pending refresh for suppressed events, or compare a generation after the operation completes. Verify edits from an editor immediately after staging or checkout.

### Medium: repository switches discard typed commit drafts

The shell calls `commitDraft.reset()` on each repository switch. Returning to the previous repository loses the typed summary and description.

Store a draft per repository. A tab switch should preserve unfinished work without prompting.

### Medium: pushing assumes the remote is named origin

`git/remote.rs` sends push and publish operations to `origin`. Pull follows Git's configured upstream. Repositories using another remote name can therefore pull successfully and fail to push.

Resolve the configured push destination and explain missing configuration. This corrects existing behavior; it does not require another remote-management feature.

### Medium: errors and in-flight states remain uneven

Some workflows still print load errors only to the console or stringify IPC error objects. Clone errors can therefore appear as `[object Object]`.

Graph subscription failures, conflict-region failures, remote-list loads, and image loads need the same visible error and superseded-request handling as status.

Long operations also need consistent protection against repeated activation. Verify double-clicking Commit, repeated keyboard shortcuts, and switching tabs during commit completion. A completion handler must not clear another repository's draft.

### Medium: normal Git configurations need explicit coverage

Repository recognition uses Git itself rather than searching for a `.git` directory. This correctly supports nested folders and linked worktrees.

Bare repositories have no working directory and currently fail the open flow with a generic repository error. Explain that limitation directly.

Remaining verification cases include submodule dirtiness, sparse checkouts, permissions failures, external repository deletion, network drives, and inherited Git environment variables. Submodule management is deferred in the roadmap, but status must still describe submodule changes accurately.

Non-UTF8 filenames cross IPC as display strings. Linux coverage proves that parsing survives; it does not prove subsequent mutations address the original filename bytes.

### Low: interface consistency still needs hands-on checks

The source contains hard-coded scrim and shadow colors despite the design-token rule. The existing contrast script checks token pairs rather than every rendered control.

Repository and clone dialogs need complete keyboard traversal, focus trapping, and focus-return checks. Graph shortcuts, popovers, and nested action controls also need native keyboard checks at minimum window size.

The clean-state panel now remains visible. Its layout and Amend discoverability should remain part of routine native regression checks.

## What is missing

BranchKit does not chiefly need more features. Its current surface already covers the central daily Git workflow.

The missing work is a repeatable native workflow suite and stronger file-preservation guarantees. Prioritize these checks:

1. Open a real repository, nested folder, linked worktree, fresh repository, and invalid folder.
2. Compare every visible status entry with Git after external edits, staging, commits, and checkout.
3. Switch repositories during delayed reads and mutations; preserve drafts and selection ownership.
4. Resolve text, binary, deletion, symlink, CRLF, and repeated rebase conflicts; inspect the resulting Git tree.
5. Discard and restore each file type; verify content, file type, staging state, and backup retention.
6. Exercise local remotes with custom names, divergence, removed upstreams, and offline failures.
7. Run the keyboard workflow and layout checks in both themes and on all supported operating systems.

Interactive rebase, an integrated terminal, and additional hosting providers remain roadmap items. They are lower priority than making existing actions predictable.

## Local check results

- Frontend: 135 tests pass; Svelte reports zero errors and zero warnings.
- Accessibility-name scan: passes for 48 Svelte files.
- Token contrast scan: all checked pairs meet AA.
- Rust: 148 unit tests and 58 integration tests pass with four download tests excluded.
- The four download tests require a local HTTP listener that the sandbox denies. 
- Clippy: passes with all targets and warnings denied.
- Native build, installation, and workflow results appear in the native verification section after execution.

## Native verification

The final packaged executable is Mach-O arm64. Its complete ad-hoc bundle signature passes `codesign --verify --deep --strict`.

Native checks used a disposable repository under `/tmp/branchkit-audit-ui`. No existing project received test commits or file changes.

| Native workflow | Observed result |
| --- | --- |
| Launch and session restoration | App opens and restores the repository tab |
| Keychain access after rebuilding | No password dialog; inaccessible saved credentials produce a visible warning |
| Open ordinary repository | Native picker opens the repository successfully |
| Partially staged file | Same file appears in both sections with the correct distinct diff |
| Nested untracked file | The file appears individually with its parent path |
| External editor change | The open diff updates to the new contents |
| Stage All and commit | Two files stage and commit; graph adds the commit and clears the draft |
| Clean repository | Status displays “Working tree clean ✓”; Amend remains visible |
| External branch checkout | Tab label, branch list, graph refs, and status header all update |
| Fresh and invalid repository through native picker | Backend fixtures pass; native picker check remains incomplete |

Native automation subsequently lost window access for both BranchKit and Finder (`cgWindowNotFound`). That prevented the remaining dialog and keyboard checks.

The native app is ready at `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/BranchKit.app`. Replacing `/Applications/BranchKit.app` requires filesystem authorization outside the workspace. The Intel app has a backup at `/tmp/BranchKit-intel-backup.app`.

GitHub and AI credentials tied to the old unsigned app may need reconnecting. The new behavior does not bypass their Keychain access controls.

## Desktop behavior review

This second pass focuses on whether BranchKit behaves like a maintained desktop application. It covers window chrome, keyboard ownership, layout, menus, and small interaction details.

### Findings and changes

| Finding | Change |
| --- | --- |
| Browser body margins create a gutter and clip the bottom of the app | Reset window margins and contain scrolling within panels |
| The document title still names the Tauri starter template | Use BranchKit and show the active repository and branch in the native window title |
| Default menus expose few BranchKit actions | Add native Open, Clone, repository tabs, Settings, and command palette commands |
| Closing the macOS window leaves no useful reopen behavior | Preserve the hidden window and restore it on Dock activation |
| Dialogs declare modality without containing focus | Contain Tab and Shift-Tab, block background shortcuts, and restore the invoking control |
| Tab close controls remain hidden during keyboard navigation | Keep active and focused close controls visible, with consistent icons and 24px targets |
| Repository tabs lack normal keyboard navigation | Add arrow keys, Home, End, and a single tab stop for the active tab |
| Context menus depend on the mouse | Add arrow navigation, submenu navigation, Escape, and focus return |
| The side panels have fixed widths despite the specification | Add persisted resizing, keyboard adjustment, and double-click reset |
| The command palette is empty without a repository | Keep Open, Clone, and Settings available |
| Palette keyboard selection can disappear outside the visible list | Scroll the selected result into view |
| The repository picker ignores Enter on a search result | Open the first matching recent repository and support arrow navigation |
| The working directory header mixes emoji and line icons | Use the existing Phosphor icons and allow controlled header wrapping |
| The credential warning dominates the toolbar | Use a compact warning with Details, a settings action, and session dismissal |
| Custom overlay colors bypass design tokens | Add shared scrim and shadow tokens for the touched dialogs and menus |

The changes retain BranchKit's specified system typography, compact density, and existing palette. This pass adds no dependencies.

### Remaining desktop quality gaps

- The branch, tag, stash, and worktree lists still contain mouse-oriented rows. They need consistent keyboard activation and context-menu access.
- Narrow graph layouts give too little width to commit messages. Column priorities and horizontal navigation need a separate layout pass.
- Loading, failure, and repeated-activation behavior varies between operations. Each long action needs a consistent busy state and completion ownership.
- Several inline popovers still need the same keyboard and focus treatment as the main dialogs.
- Settings remains an in-window dialog. Independent window behavior and saved window geometry still need deliberate product decisions.
- The broader interface still contains emoji and hard-coded shadow colors. The touched controls now follow the existing icon and token system.

### Desktop verification

Native checks confirm the active repository title, new File menu, Settings focus containment, Escape dismissal, tab arrow navigation, and context-menu arrow navigation. Closing the macOS window and reopening the application preserves both repository tabs.

A screenshot confirms that the browser gutter and bottom clipping are gone. The warning, tab close controls, and working directory actions use the updated layout.

Native checks also caught two issues during implementation: submenu commands did not receive enabled-state updates, and resize handles did not focus on pointer activation. Both received corrections before the final build.

Final native checks pass: File commands enable with an open repository and disable during a modal dialog. The File menu opens the repository picker. Arrow keys resize the focused sidebar, and Home restores its default width.

Frontend tests pass (135); Rust tests pass (206), with four sandbox-blocked download tests excluded. Clippy and Svelte checks pass without warnings. Accessibility-name and token-contrast scans pass.
