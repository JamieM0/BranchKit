<script lang="ts">
  import CloneDialog from "$lib/components/shell/CloneDialog.svelte";
  import EmptyState from "$lib/components/shell/EmptyState.svelte";
  import FirstLaunch from "$lib/components/shell/FirstLaunch.svelte";
  import RepoPicker from "$lib/components/shell/RepoPicker.svelte";
  import RepoTabs from "$lib/components/shell/RepoTabs.svelte";
  import { isModEvent } from "$lib/platform";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { repos } from "$lib/stores/repo.svelte";

  let showPicker = $state(false);
  let showClone = $state(false);

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
    if (key === "t") {
      e.preventDefault();
      showPicker = true;
    } else if (key === "w") {
      e.preventDefault();
      if (repos.activeId) void repos.close(repos.activeId);
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
    <div class="content">
      {#if repos.active}
        <div class="placeholder">
          <h2>{repos.active.name}</h2>
          <p>{repos.active.detached ? "Detached HEAD" : (repos.active.branch ?? "No commits yet")}</p>
          <p class="hint">The graph, status and diff views land in later prompts.</p>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if showPicker}
  <RepoPicker onOpenPath={handleOpenPath} onRequestClone={requestClone} onDismiss={dismissPicker} />
{/if}

{#if showClone}
  <CloneDialog onDismiss={dismissClone} />
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
    overflow: auto;
  }

  .placeholder {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    color: var(--text-muted);
  }

  .placeholder h2 {
    color: var(--text);
    font-size: 16px;
  }

  .hint {
    color: var(--text-faint);
    font-size: 12px;
  }
</style>
