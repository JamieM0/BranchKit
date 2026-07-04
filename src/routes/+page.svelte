<script lang="ts">
  import CloneDialog from "$lib/components/shell/CloneDialog.svelte";
  import EmptyState from "$lib/components/shell/EmptyState.svelte";
  import FirstLaunch from "$lib/components/shell/FirstLaunch.svelte";
  import RepoPicker from "$lib/components/shell/RepoPicker.svelte";
  import RepoTabs from "$lib/components/shell/RepoTabs.svelte";
  import GraphView from "$lib/components/graph/GraphView.svelte";
  import { isModEvent } from "$lib/platform";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { repos } from "$lib/stores/repo.svelte";
  import { graph } from "$lib/stores/graph.svelte";

  let showPicker = $state(false);
  let showClone = $state(false);

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
      }
    } else if (openedGraphId) {
      openedGraphId = null;
      void graph.close();
    }
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
        <GraphView
          onSelectCommit={(sha) => console.debug("select commit", sha)}
          onCompare={(a, b) => console.debug("compare", a, b)}
          onOpenCommit={(sha) => console.debug("open commit", sha)}
          onCheckout={(sha) => console.debug("checkout (detach)", sha)}
          onCreateBranch={(sha) => console.debug("create branch at", sha)}
          onBackToBranch={() => console.debug("back to previous branch")}
        />
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
    min-height: 0;
    overflow: hidden;
  }
</style>
