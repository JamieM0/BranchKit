<script lang="ts">
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
  import { commandPalette } from "$lib/stores/commandPalette.svelte";
  import { isModEvent } from "$lib/platform";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { repos } from "$lib/stores/repo.svelte";
  import { graph } from "$lib/stores/graph.svelte";
  import { status } from "$lib/stores/status.svelte";
  import { diffView } from "$lib/stores/diffView.svelte";
  import { fileInspector } from "$lib/stores/fileInspector.svelte";
  import { worktreeDialog } from "$lib/stores/worktreeDialog.svelte";
  import { branchEdit } from "$lib/stores/branchEdit.svelte";
  import { graphNav } from "$lib/stores/graphNav.svelte";
  import { commitDraft } from "$lib/stores/commitDraft.svelte";
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

  let showPicker = $state(false);
  let showClone = $state(false);

  // Settings + GitHub connection are app-wide, not per-repo — load once at startup.
  $effect(() => {
    void appSettings.load();
    void github.checkConnection();
    void ai.init();
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
        graph.open(id).catch((e) => console.error(e));
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
      // Routed through the shared error-toast surface once ARCHITECTURE §9 lands (prompt 10).
      console.error(e);
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!isModEvent(e)) return;
    const key = e.key.toLowerCase();
    const id = repos.activeId && !repos.activeId.startsWith("pending:") ? repos.activeId : null;
    const branch = graph.head && !graph.head.detached ? graph.head.branch : null;
    if (key === "k") {
      // Cmd+K → command palette (§10 global map).
      e.preventDefault();
      commandPalette.toggle();
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
        if (e.shiftKey) void actions.popStash(id, "stash@{0}", "");
        else void actions.stashPush(id, {});
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

{#if !onboarding.done}
  <FirstLaunch />
{:else if repos.tabs.length === 0}
  <EmptyState onOpenPath={handleOpenPath} onRequestClone={requestClone} />
{:else}
  <div class="shell">
    <RepoTabs onPick={openPicker} />
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
<CommandPalette />
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
</style>
