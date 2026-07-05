# ARCHITECTURE.md — BranchKit Technical Design

> **Audience:** the Claude Sonnet sessions building BranchKit. This file makes the hard technical
> decisions *for* you and explains the non-obvious implementation strategies. Follow it. If a
> section says "do X, not Y", that choice was made deliberately after weighing alternatives —
> do not relitigate it mid-build. Companion docs: DESIGN_SPEC.md (UI/UX), BUILD_PROMPTS.md (order).

---

## 1. Stack (final)

| Layer | Choice | Notes |
|---|---|---|
| Shell | **Tauri 2.x** (Rust backend) | Small, fast, native menus; sidecar support for llama-server |
| Frontend | **Svelte 5 + TypeScript + Vite** | Use runes (`$state`, `$derived`, `$effect`) |
| Git access | **System `git` CLI, spawned** | §3. Never libgit2, never shell strings |
| Graph render | Canvas (edges/nodes) + DOM overlay (rows/labels) | §5 |
| Syntax highlight | `highlight.js` (line-by-line) | §6.2 |
| FS watching | `notify` crate (Rust) | §4 |
| Secrets | `keyring` crate (Keychain/CredMan/libsecret) | §8 |
| Local LLM | llama.cpp **`llama-server` sidecar** over HTTP | §10 |
| GitHub | REST via `reqwest`, device-flow OAuth | §11 |
| Tests | Rust: `cargo test` w/ real-git fixtures; TS: Vitest | §12 |
| CI/Release | GitHub Actions matrix + `tauri-action` | free for public repos |

Minimum supported git: **2.30**. Detect at startup (`git --version`); show a blocking friendly
screen if missing/old with install links per OS.

**Repository layout:**
```
branchkit/
  src/                  # Svelte frontend
    lib/components/     # one folder per DESIGN_SPEC section (graph/, panel/, diff/, keep-panel/…)
    lib/stores/         # repo.svelte.ts, graph.svelte.ts, settings.svelte.ts, toasts.svelte.ts
    lib/ipc.ts          # typed invoke() wrappers — the ONLY place invoke() is called
    lib/types.ts        # mirrors Rust DTOs exactly
  src-tauri/
    src/git/            # exec.rs, log.rs, status.rs, refs.rs, diff.rs, stage.rs, conflict.rs,
                        # stash.rs, remote.rs, worktree.rs   (one module per command family)
    src/watcher.rs  src/credentials.rs  src/ai/  src/github/  src/state.rs  src/error.rs
  docs/                 # these four .md files move here in prompt 1
```

---

## 2. Process & data-flow model

- **Rust owns all git and secret operations.** The frontend never touches the filesystem, never
  builds git commands, never sees credentials.
- **Commands (frontend → Rust):** `#[tauri::command]` async fns returning
  `Result<Dto, AppError>`. All DTOs `serde`-serialized structs mirrored in `types.ts`.
  Keep commands coarse: `get_graph(repo_id, skip, limit)`, `get_status(repo_id)`,
  `stage_lines(repo_id, file, patch_spec)` — not one command per git flag.
- **Events (Rust → frontend):** `app.emit("repo://{id}/changed", ChangeKind)` where ChangeKind ∈
  `{WorkingTree, Index, Refs, Head, Remote, OperationProgress{…}}`. Frontend stores subscribe
  and re-query only what changed (WorkingTree → status only; Refs/Head → refs + graph head
  window; never a full graph reload unless topology changed).
- **Per-repo actor:** each open repo gets a `RepoHandle { id, path, op_queue, watcher }` in a
  `DashMap` in tauri State. **All mutating git commands go through the repo's serial queue**
  (a `tokio::sync::Mutex` or mpsc-worker). Read commands may run concurrently. This prevents
  index.lock collisions with ourselves. A `generation: AtomicU64` bumps on every self-initiated
  op so the watcher can skip redundant refresh events (§4).

---

## 3. Git execution layer (`git/exec.rs`) — get this right first

The single most important module. Everything else parses its output.

```rust
pub struct GitOutput { pub stdout: Vec<u8>, pub stderr: String, pub code: i32 }

pub async fn git(repo: &Path, args: &[&str], opts: GitOpts) -> Result<GitOutput, GitError>
```

Rules (non-negotiable):
1. **Args as a vector, never a shell string.** No `sh -c`. This eliminates injection and quoting
   bugs. Filenames go through as raw args; use `--` separators before pathspecs everywhere
   (`git checkout -- <file>`, `git add -- <file>`).
2. **Environment for every call:** `GIT_TERMINAL_PROMPT=0` (never hang on a password prompt),
   `LC_ALL=C` (stable parseable output), `GIT_OPTIONAL_LOCKS=0` (reads don't take locks).
   Config injection per call: `-c color.ui=false -c core.quotepath=false -c log.showSignature=false`.
3. **Timeouts:** default 30s; network ops (fetch/pull/push/clone) 10min with progress parsing
   (§3.1); kill process group on timeout.
4. **stdout is bytes, not String** — filenames and diffs can be non-UTF8. Parse with
   `String::from_utf8_lossy` only at display boundaries.
5. On nonzero exit, wrap into `GitError { code, stderr, cmd_summary }` and pass through the
   error translator (§9) before it reaches the UI.
6. **Windows:** spawn with `CREATE_NO_WINDOW` flag; find git via `where git` fallback to
   known install paths. Never assume a POSIX shell exists.

### 3.1 Progress for long ops
Spawn with `--progress` (fetch/push/clone report on stderr). Read stderr incrementally
(BufReader on the child's stderr, split on `\r` as well as `\n`), regex the percentages
(`Receiving objects:\s+(\d+)%`), emit `OperationProgress` events. This is what makes clone/fetch
feel alive.

---

## 4. Filesystem watching (`watcher.rs`)

- `notify` recommended-watcher on **two scopes**: `.git` dir (HEAD, refs/, packed-refs, index,
  MERGE_HEAD, REBASE_HEAD, CHERRY_PICK_HEAD, FETCH_HEAD) and the worktree root (recursive).
- **Debounce 300ms** (collect events, fire once). Classify: paths under `.git` → Index/Refs/Head
  kinds by filename; else WorkingTree.
- **Ignore:** `.git/index.lock`, `*.lock` transients, and anything matching gitignore for
  worktree events — cheapest correct approach: don't filter worktree events by ignore rules at
  all; just debounce and let `git status --porcelain=v2` be the authority (it's fast). Only
  guard against event storms: if >500 events in a window (branch checkout, build output), fold
  into one refresh.
- **Self-echo suppression:** before each mutating op, bump repo `generation` and record a 1.5s
  suppression window; watcher events inside the window for kinds the op itself causes are
  dropped (the op's completion handler triggers the refresh instead, so the UI updates exactly
  once, immediately).

---

## 5. Commit graph pipeline

### 5.1 Data
```
git log --all --topo-order --date-order-fallback? NO — use: 
git log --branches --remotes --tags HEAD --topo-order \
  --pretty=format:%H%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s%x1f%b%x1e -z? NO — %x1e record sep only
```
Use `%x1f` (unit sep) between fields, `%x1e` (record sep) between commits; body (`%b`) is LAST
field so embedded newlines are safe (records split on `%x1e`, not newlines). Take only the first
line of `%b` for the graph preview; full body lazy-loaded on selection via `git show -s`.
Stashes: `git stash list --pretty=…` gives `sha, parent(base) sha, message`; inject as pseudo-rows
attached to their base commit. Refs come separately (§5.3) and are overlaid by sha.

Paginate: initial `-n 2000`, then background chunks of 5000 using `--skip` is O(n²) — instead
paginate by commit boundary: remember last sha of previous chunk and continue with
`git log … <last_sha>~1` per root… simpler and correct: fetch the FULL sha+parents list once
(`git rev-list --all --topo-order --parents` — this is fast even at 100k, ~a few MB) to build
topology, and lazy-load per-commit metadata (author/subject) in visible-window batches via
`git show --quiet --pretty=…` batched 200 shas per call. Topology once + metadata on demand =
snappy at any size. **Do it this way.**

### 5.2 Lane assignment (run in TS, it's pure; unit-test heavily)
Classic active-lanes algorithm over the topo-ordered list:
```
lanes: (sha|null)[] = []          // lanes[i] = sha this lane is waiting to meet
for each commit c (top to bottom):
  expecting = all i where lanes[i] == c.sha
  if expecting empty: c.lane = first null slot ?? lanes.push  // a tip
  else: c.lane = min(expecting); others in expecting merge-curve into c.lane, set null
  lanes[c.lane] = c.parents[0] ?? null                        // continue first parent
  for p in c.parents[1..]:
    j = lanes.indexOf(p)          // parent already expected → curve to j
    if j == -1: j = first null ?? push; lanes[j] = p          // open new lane
    record edge (c.row, c.lane) → (rowOf(p) when reached, j)
```
Emit per-row draw ops: `{node(lane), edges:[{fromLane,toLane,kind:pass|fork|merge}]}`. Color =
`palette[lane % 8]`. Compact lanes: prefer reusing null slots left of current max. Snapshot-test
against fixture repos (linear, one merge, octopus, criss-cross).

### 5.3 Refs & ahead/behind
```
git for-each-ref --format='%(refname)%1f%(objectname)%1f%(upstream:short)%1f%(upstream:track)%1f%(HEAD)' \
  refs/heads refs/remotes refs/tags
```
`%(upstream:track)` gives `[ahead 2, behind 1]` — parse it; it's computed by git, always
consistent, no extra rev-list calls. Refresh refs after every fetch/mutation. Current branch +
detached state: `git symbolic-ref -q HEAD` / `git rev-parse HEAD`.

### 5.4 Rendering (the hybrid — do NOT go full-canvas or full-DOM)
- **DOM layer:** a virtualized list (write it yourself — ~80 lines: container with
  `transform: translateY`, render visibleRange ± 20 overscan; don't add a library) of row divs:
  message, badges, ref pills — all normal Svelte components, focusable, right-clickable.
- **Canvas layer:** one `<canvas>` positioned behind the rows, sized to viewport,
  `devicePixelRatio`-scaled, redrawn on scroll via rAF: draws only edges + node discs for the
  visible range. Bezier for fork/merge (control points at ±rowHeight/2). Avatars: draw clipped
  circles from a decoded-ImageBitmap LRU cache; fallback initials disc drawn in canvas.
- Hit-testing for nodes = simple math (row from y, lane from x). Everything else is DOM.
- Scroll anchoring (DESIGN_SPEC §4.7): on data refresh, find previously-topmost sha's new index
  and restore scrollTop so it stays put.

---

## 6. Status, diffs, staging

### 6.1 Status
`git status --porcelain=v2 --branch -z`. Parse: `1 XY … path` (ordinary), `2 XY … path\0origPath`
(rename), `u XY … path` (unmerged), `? path` (untracked). XY: X=index/staged state,
Y=worktree/unstaged state — one file can appear in both UI sections (partially staged) when both
are non-`.`. `-z` = NUL-terminated, no quoting issues. The `--branch` header lines give
oid/upstream/ab as a bonus cross-check.

### 6.2 Diffs
- Worktree vs index: `git diff -- <file>`; index vs HEAD: `git diff --cached -- <file>`;
  commit: `git show <sha> -- <file>`; two commits: `git diff <a> <b> -- <file>`.
  Always `--no-color -U3`; binary detection via `Binary files … differ` line + a null-byte sniff.
- Parse unified diff into `Hunk { header, lines: [{kind: ctx|add|del, oldNo, newNo, text}] }` in
  Rust; ship structured to frontend. Intra-line word diff: compute in TS between paired add/del
  lines with a small LCS on word tokens (write it; ~40 lines; test it).
- Highlighting: run highlight.js **per full line** with the language from extension; for
  correctness across multi-line constructs use `highlight(text, {language})` on the joined hunk
  then split — acceptable v1 compromise; do not pull in CodeMirror for the diff view.

### 6.3 Hunk & line staging — the exact technique
To stage a subset of changes, construct a patch and apply to the index:
1. Get the file's full worktree-vs-index diff (that's the unstaged set).
2. Build a new patch containing only selected hunks. For **line-level**: within a hunk, keep
   selected `+`/`-` lines as-is; convert **unselected `-` lines to context** (they must remain
   in the result); **drop unselected `+` lines entirely**.
3. Don't hand-compute hunk header counts — emit approximately and run
   `git apply --cached --recount --unidiff-zero=NO --whitespace=nowarn -` feeding the patch on
   stdin. `--recount` makes git fix the counts.
4. **Unstage** = same patch applied with `--reverse --cached`.
5. Edge cases to handle explicitly (test each): file with `\ No newline at end of file`
   (preserve the marker with its line); untracked files (stage whole via `git add` —
   no partial); renames (partial staging disabled; whole-file only); mode-only changes
   (whole-file only); binary (whole-file only). When constructing from the *staged* side
   (unstaging lines), diff base is `--cached`.

---

## 7. Operations

### 7.1 Mutations (all through the op queue; all trigger targeted refresh on completion)
commit (`git commit -m <s> [-m <body>] [--amend]` — pass message via `-m` args, not tempfile,
unless >8k chars → `-F -` stdin) · checkout branch (`git checkout <name>` / new tracking:
`git checkout -b <name> --track <remote>/<name>`) · previous (`git checkout -`) · detached
(`git checkout --detach <sha>`) · branch create/delete (`-d`, force `-D` after guard)/rename
(`-m`) · merge (`git merge --no-edit <ref>`, abort `--abort`) · rebase onto
(`git rebase <sha>`) · cherry-pick / revert (with `--no-commit` option per DESIGN_SPEC) · reset
(`--soft|--mixed|--hard <sha>`) · tag (`git tag [-a -m]`) · stash
(`git stash push [-u] [-m]` / `apply|pop stash@{n}` / `drop`) · fetch
(`git fetch --all --prune --progress`) · pull (ff: `--ff-only`, rebase: `--rebase`, merge:
`--no-rebase`) · push (`git push origin <branch>`, publish: `-u origin <name>`, force:
**`--force-with-lease` always — never `--force`**) · worktree (`git worktree add <path> <ref>`,
`list --porcelain`, `remove`, `prune`).

### 7.2 Auto-fetch
Tokio interval per repo (setting; default 1min), **only while window focused** (listen to tauri
focus events), skipped if an op is queued/running or a fetch ran <30s ago (manual counts).
Emits Remote change events → refs refresh → badge updates → optional toast (DESIGN_SPEC §8).

### 7.3 Discard safety net
Before any discard: write `git diff -- <files>` output (and full copies of affected untracked
files, zipped) to `app_data/trash/<repo-hash>/<ISO-timestamp>/` with a small `manifest.json`
(files, op description). Then `git restore --worktree -- <files>` (+ delete untracked). Restore
= `git apply` the patch / unzip untracked. Purge >7 days on app start. Cap trash at 200MB per
repo (evict oldest).

### 7.4 Conflict state detection
After any op and on watcher Head events, probe: `.git/MERGE_HEAD` (merge) / `REBASE_HEAD` or
`rebase-merge/` dir (rebase) / `CHERRY_PICK_HEAD` / `REVERT_HEAD`; unmerged paths from status
`u` lines. Expose `ConflictState { kind, source_label, target_label, files }`. Labels: target =
current branch; source = for merge read `.git/MERGE_MSG` first line or resolve MERGE_HEAD sha to
a ref name (`git name-rev --name-only`); for stash-apply conflicts label "stash".
Continue = `git commit --no-edit` (merge) / `git rebase --continue` / `git cherry-pick
--continue` (set `GIT_EDITOR=true` so git never opens an editor). Abort = corresponding `--abort`.

### 7.5 Keep Panel data model
For each conflicted file, read the three stages: `git show :1:<path>` (base), `:2:` (ours),
`:3:` (theirs). **Do not parse worktree conflict markers** — marker parsing breaks on files
containing marker-like text and on diff3 style. Instead compute regions yourself:
1. `diff(base, ours)` and `diff(base, theirs)` (use the `similar` crate, Myers).
2. Walk base line-by-line; where both sides' changed regions overlap (touch the same base span)
   → **conflict region** `{ base_span, ours_lines, theirs_lines }`; where only one side changed
   → auto-resolved lines (take that side); unchanged → context.
   This is a simplified 3-way merge — ~150 lines, fully unit-testable, and gives you exact
   ours/theirs line arrays with no marker ambiguity. Pre-dedupe identical ours/theirs lines at
   region edges (DESIGN_SPEC §9.3).
3. Frontend state per region: `candidates` + `kept: [{source: ours|theirs|edit, lines, order}]`.
   Resolved file text = context + auto-resolved + kept (in click order) — renumber reactively.
4. Confirm: write resolved text to the worktree file, `git add -- <path>`. Reopen ("Reset
   file"): `git checkout -m -- <path>` regenerates the conflict.

---

## 8. Credential manager (Jamie: "careful with security!")

Threat model: never store secrets in config/plaintext; never log them; never send anywhere
except the git host; frontend never sees them (only metadata: host, username, last-used).

- **Storage:** `keyring` crate — service `"BranchKit"`, account `"{host}:{username}"`.
- **Supplying creds to git — use the credential-helper protocol, injected per call:** every
  network command gets `-c credential.helper=` (empty — clears system helpers) and
  `-c credential.helper=!<path-to-branchkit-binary> credential-helper` .
  Implement the `credential-helper` CLI subcommand in the same binary (tauri lets you handle
  argv early, before window creation): it speaks git's stdin/stdout protocol —
  `get` → look up keyring by host, print `username=…\npassword=…`; `store` → save on success;
  `erase` → remove on auth failure. If `get` misses, the main app (notified via a local socket
  or simply by the command failing with our sentinel) prompts the user with a credential dialog,
  saves, retries the operation once. Simpler v1 fallback (acceptable): on 401/`Authentication
  failed` stderr, show the dialog, store to keyring, retry — and let the helper find it.
- **SSH:** delegate to the user's ssh-agent/keys (do not manage passphrases). Detect agent
  (`ssh-add -l`). "Generate key" helper runs `ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519_branchkit
  -N ""` only with explicit user consent about the empty passphrase (offer passphrase input,
  passed via stdin, zeroized after). Show pubkey with copy button + link to GitHub keys page.
- GitHub OAuth token: keyring, `account:"github-oauth"`. API keys for AI: keyring,
  `account:"ai-api-key"`. Config files store only non-secret settings.
- Mask everything in UI (`•••• last4`), `Zeroizing<String>` for in-memory secrets in Rust,
  and audit: `grep -ri password\|token` over logs in CI test.

---

## 9. Error translation (`error.rs`)

`AppError { user_message, suggestion: Option<{label, action_id}>, raw: String, kind }`.
Match stderr patterns → catalog (extend as found):
`non-fast-forward|fetch first` → behind-remote + Pull suggestion · `index.lock` → another
process + Retry · `Authentication failed|403` → credentials + open-settings ·
`Could not resolve host` → offline (also flip a global offline indicator; retry fetches on
focus) · `would be overwritten by checkout` → uncommitted changes + Stash-and-checkout action
(runs stash→checkout→pop sequence) · `no upstream` → Publish suggestion · `unrelated histories`
→ explain + allow-unrelated option. Unknown errors: generic sentence + Details. Every dialog’s
Details section shows the sanitized command + stderr (never credentials — scrub URL userinfo).

---

## 10. AI layer (`ai/`)

**One internal interface, three providers, all speaking HTTP chat-completions:**
`AiProvider { list_models?, generate(messages, on_token) }`.
- **Local (first-class):** manage a llama.cpp **`llama-server`** binary + GGUF model in app data.
  - Downloads (both with resume via Range, sha256 verify, progress events): llama-server from
    a **pinned** ggml-org/llama.cpp release per OS/arch (document exact asset names in a
    `versions.rs`); model `gemma-3-1b-it-Q4_K_M.gguf` from a pinned HF URL (ggml-org or
    unsloth mirror; record sha256 at pin time).
  - Runtime: spawn on first generate: `llama-server -m <model> --port 0→pick free port -c 4096
    --host 127.0.0.1`, poll `/health`, keep-alive 5min idle then kill; kill on app exit
    (register in tauri on_exit; also take a pidfile to reap orphans on next start).
  - Chat via OpenAI-compatible `POST /v1/chat/completions` with `stream:true` (SSE) —
    identical client code to Remote-OpenAI. 1B model note: keep prompts short and structured.
- **Ollama:** base URL setting; `GET /api/tags` → models; generate via its OpenAI-compat
  `/v1/chat/completions`. Connection dot = 2s-timeout ping.
- **Remote API:** OpenAI format (base URL + `/chat/completions`, `Authorization: Bearer`) or
  Anthropic format (`/v1/messages`, `x-api-key`, `anthropic-version` header, different
  request/response shape — implement as a thin adapter to the same interface). Stream both.
- **Commit-message prompt:** system: "Write a git commit message. First line ≤72 chars,
  imperative mood[, Conventional Commits format]. Then a blank line and 1–4 bullet body only if
  the change is non-trivial. Output nothing else." User content: `git diff --cached --stat`
  plus the diff truncated: per-file cap 150 lines, total cap ~8k chars, note "(N more files)"
  when truncated. Parse: first line → summary field, rest → description. Stream into the UI.

---

## 11. GitHub (`github/`)

- **Auth:** OAuth **device flow** (no client secret needed; the client_id ships in source —
  that is safe and normal). *Jamie must register the OAuth app (github.com → Settings →
  Developer settings → OAuth Apps → enable Device Flow) and put the client_id in `github/mod.rs`.*
  Flow: `POST https://github.com/login/device/code` (scope `repo`) → show user_code →
  poll `POST /login/oauth/access_token` at `interval` (respect `slow_down`) → keyring.
- **API:** `reqwest` + serde structs (no octocrab — fewer deps, we need 6 endpoints):
  `GET /user` (identity) · `GET /repos/{o}/{r}/pulls?state=open&per_page=50` (map `head.ref` →
  branch badges) · `GET /repos/{o}/{r}/commits/{sha}/check-runs` + `…/status` (CI dots —
  lazy: only visible commits, cache 60s, batch on scroll-idle) · `POST /pulls` (create) ·
  `PUT /pulls/{n}/merge` · checkout PR head: `git fetch origin pull/{n}/head:pr-{n}` then
  checkout (works for forks).
- Repo detection: parse `origin` URL (ssh `git@github.com:o/r.git` and https forms) → owner/repo;
  non-GitHub remotes → integration surfaces simply don't render.
- Rate limits: read `x-ratelimit-remaining`, back off <10, never poll checks more than 1/min/repo.

---

## 12. Testing & CI

- **Rust integration tests are the backbone:** helper `TestRepo` builds real repos in tempdirs
  by running git (`init`, scripted commits/branches/merges/conflicts). Every parser and every
  operation gets tests against real git output — this is what keeps a git client trustworthy.
  Required fixtures: linear history · merge · criss-cross · renames · partial staging each edge
  case in §6.3(5) · conflict files incl. marker-like content (`<<<<<<<` inside code) ·
  no-trailing-newline · non-UTF8 filename (skip on Windows) · detached HEAD · worktrees.
- **TS (Vitest):** lane algorithm snapshots, word-diff, keep-panel region reducer
  (keep/unkeep/reorder/renumber), stores.
- **CI (GitHub Actions):** `test.yml` on push/PR: matrix {macos, windows, ubuntu} → cargo test,
  vitest, `cargo clippy -D warnings`, `svelte-check`. `release.yml` on tag: tauri-action builds
  dmg/msi/AppImage+deb, attaches to a GitHub Release. Ubuntu needs the webkit2gtk system deps
  step (document the apt list in the workflow).
- No E2E harness in v1 (Tauri E2E is flaky); compensate with the integration-test layer and
  manual QA checklists in the build prompts.

---

## 13. Performance budgets & tactics

startup→graph <1.5s (5k repo) · watcher→UI <1s · scroll 60fps @20k · keypress→paint <50ms.
Tactics: topology-once + metadata-on-demand (§5.1) · single canvas redraw per rAF · avatar
ImageBitmap LRU (200) · status/diff parsing in Rust, not JS · never `git log` unbounded ·
check-runs only for visible rows · debounce filter input 150ms · virtualize every list
(graph, file lists >100, blame).

---

## 14. Cross-platform gotchas (Sonnet: read twice)

Windows: `CREATE_NO_WINDOW`; paths with `\` — always use `std::path`, display with forward
slashes in UI; `core.autocrlf` noise → show a quiet "line endings" note in diffs rather than
full-file changes when only EOL differs (detect: diff with `--ignore-cr-at-eol` empty).
macOS: app nap — auto-fetch already pauses on blur; codesign/notarize deferred (document in
release prompt). Because of that deferral, `tauri dev`'s unsigned debug binary has no stable
identity across rebuilds, so Keychain re-prompts for the GitHub/AI secrets (credentials.rs) on
every launch even after "Always Allow" — `scripts/dev-cert-setup.sh` + `npm run
tauri:dev:signed` work around this for local dev by signing with a stable self-signed identity;
this is unrelated to the real release signing/notarization, which is still deferred. Linux:
keyring needs libsecret/gnome-keyring — degrade with in-memory-only secrets + a warning banner
if unavailable. All: never assume `~`, use tauri path APIs.
