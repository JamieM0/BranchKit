<script lang="ts">
  import { X } from "phosphor-svelte";
  import CloneDialog from "$lib/components/shell/CloneDialog.svelte";
  import CreateWorktreeDialog from "$lib/components/shell/CreateWorktreeDialog.svelte";
  import EmptyState from "$lib/components/shell/EmptyState.svelte";
  import FirstLaunch from "$lib/components/shell/FirstLaunch.svelte";
  import RepoPicker from "$lib/components/shell/RepoPicker.svelte";
  import RepoTabs from "$lib/components/shell/RepoTabs.svelte";
  import Toolbar from "$lib/components/shell/Toolbar.svelte";
  import ConflictBanner from "$lib/components/keep-panel/ConflictBanner.svelte";
  import KeepPanel from "$lib/components/keep-panel/KeepPanel.svelte";
  import GraphView from "$lib/components/graph/GraphView.svelte";
  import LeftPanel from "$lib/components/panel/LeftPanel.svelte";
  import RightPanel from "$lib/components/panel/RightPanel.svelte";
  import DiffViewer from "$lib/components/diff/DiffViewer.svelte";
  import FileInspector from "$lib/components/diff/FileInspector.svelte";
  import ToastStack from "$lib/components/shell/ToastStack.svelte";
  import CommandPalette from "$lib/components/shell/CommandPalette.svelte";
  import CredentialDialog from "$lib/components/shell/CredentialDialog.svelte";
  import SettingsWindow from "$lib/components/settings/SettingsWindow.svelte";
  import { credentialDialog } from "$lib/stores/credentialDialog.svelte";
  import { commandPalette } from "$lib/stores/commandPalette.svelte";
  import { isModEvent } from "$lib/platform";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { repos } from "$lib/stores/repo.svelte";
  import { graphSelection } from "$lib/stores/graphSelection.svelte";
  import { graph } from "$lib/stores/graph.svelte";
  import { status } from "$lib/stores/status.svelte";
  import { diffView } from "$lib/stores/diffView.svelte";
  import { fileInspector } from "$lib/stores/fileInspector.svelte";
  import { worktreeDialog } from "$lib/stores/worktreeDialog.svelte";
  import { branchEdit } from "$lib/stores/branchEdit.svelte";
  import { graphNav } from "$lib/stores/graphNav.svelte";
  import { commitDraft } from "$lib/stores/commitDraft.svelte";
  import { unstagedRows, stagedRows } from "$lib/status/sections";
  import { keepSession } from "$lib/stores/keepSession.svelte";
  import { network, retryOnFocus } from "$lib/stores/network.svelte";
  import { notifyBehindIncrease, resetBehindTracking } from "$lib/stores/behindNotifier";
  import { appSettings } from "$lib/stores/appSettings.svelte";
  import { ai } from "$lib/stores/ai.svelte";
  import { settingsWindow } from "$lib/stores/settingsWindow.svelte";
  import { github } from "$lib/stores/github.svelte";
  import { githubChecks } from "$lib/stores/githubChecks.svelte";
  import { prPanel } from "$lib/stores/prPanel.svelte";
  import { createPrDraft } from "$lib/stores/createPrDraft.svelte";
  import * as actions from "$lib/actions";
  import { onDesktopAction, pickFolder, setWindowContext, openInBrowser, checkGitAvailable, credentialStorageStatus } from "$lib/ipc";
  import { toasts } from "$lib/stores/toasts.svelte";

  let gitReady = $state(false);
  let gitError = $state<string | null>(null);
  async function checkGit() {
    gitError = null;
    try { await checkGitAvailable(); gitReady = true; }
    catch (error) { gitError = actions.asAppError(error).userMessage; }
  }
  $effect(() => { void checkGit(); });

  let showPicker = $state(false);
  let showClone = $state(false);
  let credentialWarning = $state<string | null>(null);
  let credentialWarningDismissed = $state(false);
  let sessionRestoreStarted = false;

  $effect(() => {
    const tab = repos.active;
    const context = tab ? `${tab.name}${tab.detached ? " · Detached HEAD" : tab.branch ? ` · ${tab.branch}` : ""} — BranchKit` : "BranchKit";
    document.title = context;
    void setWindowContext(context, !!tab && !tab.cloneProgress, showPicker || showClone || settingsWindow.open || commandPalette.isOpen || credentialDialog.state !== null || worktreeDialog.startRef !== null).catch(() => {});
  });

  $effect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onDesktopAction((action) => {
      if (action === "command_palette" && commandPalette.isOpen) { commandPalette.close(); return; }
      // Native accelerators must respect modal ownership just like keyboard events do.
      if (document.querySelector('[aria-modal="true"]')) return;
      if (action === "settings") settingsWindow.show();
      else if (action === "command_palette") commandPalette.toggle();
      else if (action === "repo_picker") openPicker();
      else if (action === "clone_repo") requestClone();
      else if (action === "close_repo" && repos.activeId) void repos.close(repos.activeId);
      else if (action === "open_repo") {
        void pickFolder("Open Repository").then((path) => { if (path) void handleOpenPath(path); });
      }
    }).then((stop) => { if (disposed) stop(); else unlisten = stop; });
    return () => { disposed = true; unlisten?.(); };
  });

  // Settings + GitHub connection are app-wide, not per-repo — load once at startup.
  $effect(() => {
    void appSettings.load();
    void Promise.all([github.checkConnection(), ai.init()]).then(() => credentialStorageStatus()).then(
      (status) => (credentialWarning = status.warning),
      () => (credentialWarning = null),
    );
  });

  $effect(() => {
    if (
      sessionRestoreStarted ||
      !gitReady ||
      !onboarding.done ||
      !appSettings.loaded ||
      !appSettings.current.general.openLastReposOnLaunch
    ) return;
    sessionRestoreStarted = true;
    void repos.restoreLastSession().then((failures) => {
      if (failures.length > 0) {
        toasts.pushError(
          `Couldn't reopen ${failures.length} saved ${failures.length === 1 ? "repository" : "repositories"}`,
          failures.join("\n"),
        );
      }
    });
  });

  // ARCHITECTURE.md §9/§14: once offline, retry a fetch as soon as the window regains focus
  // rather than waiting for the next auto-fetch tick.
  $effect(() => {
    return retryOnFocus(() => {
      if (repos.activeId && !repos.activeId.startsWith("pending:")) {
        void actions.fetchAll(repos.activeId);
      }
    });
  });

  // Keep the graph store pointed at the active repo. Real repo ids only — a `pending:` clone tab has
  // no backend repo yet. Git mutations from the graph (checkout, create branch, …) land in later
  // prompts; for now the graph emits them as intents that this shell logs.
  let openedGraphId: string | null = null;
  $effect(() => {
    const id = repos.activeId;
    if (id && !id.startsWith("pending:")) {
      if (id !== openedGraphId) {
        openedGraphId = id;
        graphSelection.clear();
        graph.open(id, repos.active?.path ?? id).catch((e) => console.error(e));
        status.open(id).catch((e) => console.error(e));
        keepSession.open(id).catch((e) => console.error(e));
        diffView.close();
        fileInspector.close();
        // A half-typed commit draft shouldn't follow you into another repo (§7).
        commitDraft.reset();
        prPanel.close();
        createPrDraft.reset();
        githubChecks.reset();
      }
    } else if (openedGraphId) {
      resetBehindTracking(openedGraphId);
      openedGraphId = null;
      void graph.close();
      void status.close();
      void keepSession.close();
      diffView.close();
      fileInspector.close();
      commitDraft.reset();
      prPanel.close();
      createPrDraft.reset();
      github.reset();
      githubChecks.reset();
    }
  });

  // A working-tree/staged diff can outlive the change it's showing (discard, commit, stage/
  // unstage elsewhere) — status refreshes independently of the diff pane, so fall back to the
  // graph instead of stranding the user on a stale "No changes" pane once the path is gone.
  $effect(() => {
    const target = diffView.target;
    if (!target) return;
    if (target.source.kind !== "workingTree" && target.source.kind !== "staged") return;
    const rows = target.source.kind === "staged" ? stagedRows(status.report.entries) : unstagedRows(status.report.entries);
    if (!rows.some((r) => r.path === target.path)) diffView.close();
  });

  // DESIGN_SPEC.md §8/§15.19: "`branch` is N behind — Pull" toast whenever a fetch (manual or
  // auto) finds new commits on the current branch's upstream.
  $effect(() => {
    const id = repos.activeId;
    const branch = graph.head && !graph.head.detached ? graph.head.branch : null;
    if (!id || id.startsWith("pending:") || !branch) return;
    const ref = graph.refs.find((r) => r.kind === "branch" && r.isHead);
    notifyBehindIncrease(id, branch, ref?.behind ?? 0);
  });

  function openPicker() {
    showPicker = true;
  }

  function dismissPicker() {
    showPicker = false;
  }

  function requestClone() {
    showPicker = false;
    showClone = true;
  }

  function dismissClone() {
    showClone = false;
  }

  async function handleOpenPath(path: string) {
    showPicker = false;
    try {
      await repos.open(path);
    } catch (e) {
      const err = actions.asAppError(e);
      toasts.pushError(err.userMessage, err.raw);
    }
  }

  /** True when the event originates from a text-editing surface — global app shortcuts must not
   * fire while the user is typing (Cmd+S in a commit summary should never stash). Cmd+K stays
   * live everywhere: the palette is the escape hatch, and opening it can't destroy typed work. */
  function isEditableTarget(e: KeyboardEvent): boolean {
    const t = e.target;
    if (!(t instanceof HTMLElement)) return false;
    return (
      t instanceof HTMLInputElement ||
      t instanceof HTMLTextAreaElement ||
      t.isContentEditable
    );
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || !isModEvent(e)) return;
    const key = e.key.toLowerCase();
    if (isEditableTarget(e) && key !== "k") return;
    const id = repos.activeId && !repos.activeId.startsWith("pending:") ? repos.activeId : null;
    const branch = graph.head && !graph.head.detached ? graph.head.branch : null;
    if (key === "k") {
      // Cmd+K → command palette (§10 global map).
      e.preventDefault();
      commandPalette.toggle();
    } else if (key === "o") {
      e.preventDefault();
      if (e.shiftKey) requestClone();
      else void pickFolder("Open Repository").then((path) => { if (path) void handleOpenPath(path); });
    } else if (key === "t") {
      e.preventDefault();
      showPicker = true;
    } else if (key === "w") {
      e.preventDefault();
      if (repos.activeId) void repos.close(repos.activeId);
    } else if (key === "b") {
      // Cmd+B → create a branch at HEAD via the inline editor (§10 global map).
      const headSha = graph.head?.sha;
      if (headSha) {
        e.preventDefault();
        graphNav.scrollTo(headSha);
        branchEdit.startCreate(headSha);
      }
    } else if (key === "p") {
      if (id && branch) {
        e.preventDefault();
        if (e.shiftKey) void actions.push(id, false, branch);
        else void actions.pull(id, appSettings.current.git.defaultPullMode, branch);
      }
    } else if (key === "s") {
      if (id) {
        e.preventDefault();
        if (e.shiftKey) void actions.popStash(id, "stash@{0}");
        else void actions.stashPush(id, {});
      }
    } else if (key === "enter") {
      if (id && commitDraft.canCommit && status.report.entries.length > 0) {
        e.preventDefault();
        void actions.commitPrimary(id);
      }
    } else if (key === ",") {
      // Cmd+, → Settings (§10 global map).
      e.preventDefault();
      settingsWindow.show();
    } else if (/^[1-9]$/.test(key)) {
      e.preventDefault();
      repos.switchToIndex(Number(key));
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if !gitReady}
  <main class="git-check">
    <h1>{gitError ? "Git needs attention" : "Checking Git…"}</h1>
    {#if gitError}
      <p>{gitError}</p>
      <p>BranchKit needs Git 2.30 or newer. Install Git, then retry.</p>
      <p><button type="button" onclick={() => openInBrowser("https://git-scm.com/downloads")}>Install Git for macOS, Windows, or Linux</button></p>
      <button type="button" onclick={checkGit}>Retry</button>
    {/if}
  </main>
{:else if !onboarding.done}
  <FirstLaunch />
{:else if repos.tabs.length === 0}
  <EmptyState onOpenPath={handleOpenPath} onRequestClone={requestClone} />
{:else}
  <div class="shell">
    <RepoTabs onPick={openPicker} />
    {#if credentialWarning && !credentialWarningDismissed}
      <div class="credential-banner" role="status">
        <span>Saved credentials are unavailable.</span>
        <button type="button" onclick={() => settingsWindow.show("credentials")}>Review credentials</button>
        <details><summary>Details</summary><p>{credentialWarning}</p></details>
        <button type="button" class="dismiss-warning" title="Dismiss for this session" aria-label="Dismiss credential warning" onclick={() => { credentialWarningDismissed = true; }}><X size={13} aria-hidden="true" /></button>
      </div>
    {/if}
    {#if network.offline}
      <div class="offline-banner" role="status">
        BranchKit can't reach the network — changes here still work, sync will resume once
        you're back online.
      </div>
    {/if}
    {#if repos.active}
      <Toolbar repoId={repos.active.id} />
      <ConflictBanner repoId={repos.active.id} />
    {/if}
    <div class="content">
      {#if repos.active}
        <LeftPanel />
        <div class="graph-area">
          {#if keepSession.panelOpen}
            <KeepPanel />
          {:else if fileInspector.target}
            <FileInspector path={fileInspector.target.path} onBack={() => fileInspector.close()} />
          {:else if diffView.target}
            <DiffViewer target={diffView.target} onBack={() => diffView.close()} />
          {:else}
            <GraphView
              onSelectCommit={(sha) => console.debug("select commit", sha)}
              onCompare={(a, b) => console.debug("compare", a, b)}
              onOpenCommit={(sha) => console.debug("open commit", sha)}
            />
          {/if}
        </div>
        <RightPanel />
      {/if}
    </div>
  </div>
{/if}

<ToastStack />
<CommandPalette onOpen={openPicker} onClone={requestClone} />
<CredentialDialog />
<SettingsWindow />

{#if showPicker}
  <RepoPicker onOpenPath={handleOpenPath} onRequestClone={requestClone} onDismiss={dismissPicker} />
{/if}

{#if showClone}
  <CloneDialog onDismiss={dismissClone} />
{/if}

{#if worktreeDialog.startRef !== null}
  <CreateWorktreeDialog startRef={worktreeDialog.startRef} onDismiss={() => worktreeDialog.close()} />
{/if}

<style>
  .git-check {
    height: 100vh;
    display: grid;
    place-content: center;
    gap: var(--space-3);
    color: var(--text);
    background: var(--bg);
  }

  .shell {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .graph-area {
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .offline-banner {
    padding: var(--space-1) var(--space-3);
    font-size: 12px;
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, var(--surface));
    border-bottom: 1px solid var(--border);
  }

  .credential-banner {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    position: relative;
    flex-shrink: 0;
    padding: var(--space-1) var(--space-3);
    font-size: 12px;
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 12%, var(--surface));
    border-bottom: 1px solid var(--border);
  }
  .credential-banner button, .credential-banner summary {
    color: var(--warn);
    background: transparent;
    border: none;
    font: inherit;
    cursor: pointer;
    padding: 2px 0;
  }
  .credential-banner button:hover { text-decoration: underline; }
  .credential-banner details p {
    position: absolute;
    top: 100%;
    left: var(--space-3);
    max-width: min(480px, 90vw);
    margin: var(--space-1) 0;
    padding: var(--space-3);
    color: var(--text);
    background: var(--overlay);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    box-shadow: 0 4px 16px var(--shadow-color);
    z-index: 60;
  }
  .credential-banner .dismiss-warning { margin-left: auto; display: grid; place-items: center; min-width: 24px; min-height: 24px; }
</style>
