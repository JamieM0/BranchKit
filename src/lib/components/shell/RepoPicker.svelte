<script lang="ts">
  import { fuzzyFilter } from "$lib/fuzzy";
  import { listRecents, pickFolder } from "$lib/ipc";
  import type { RecentRepo } from "$lib/types";

  let {
    onOpenPath,
    onRequestClone,
    onDismiss,
  }: {
    onOpenPath: (path: string) => void;
    onRequestClone: () => void;
    onDismiss: () => void;
  } = $props();

  let query = $state("");
  let recents: RecentRepo[] = $state([]);
  let inputEl: HTMLInputElement | undefined = $state();

  $effect(() => {
    listRecents().then((r) => {
      recents = r;
    });
  });

  $effect(() => {
    inputEl?.focus();
  });

  let filteredRecents = $derived(fuzzyFilter(query, recents, (r) => `${r.name} ${r.path}`));

  async function handleOpenBrowse() {
    const path = await pickFolder("Open Repository");
    if (path) onOpenPath(path);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onDismiss();
    }
  }

  function handleScrimClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onDismiss();
  }
</script>

<div class="scrim" onclick={handleScrimClick} onkeydown={handleKeydown} role="presentation">
  <div class="picker" role="dialog" aria-modal="true" aria-label="Open or clone a repository">
    <input
      bind:this={inputEl}
      bind:value={query}
      onkeydown={handleKeydown}
      type="text"
      placeholder="Search recent repositories…"
      class="search"
      aria-label="Search recent repositories"
    />
    <div class="results">
      <button type="button" class="row action" onclick={handleOpenBrowse}>
        <span class="icon" aria-hidden="true">📂</span> Open…
      </button>
      <button type="button" class="row action" onclick={onRequestClone}>
        <span class="icon" aria-hidden="true">⬇</span> Clone…
      </button>
      {#if recents.length > 0}
        <div class="section-label">Recent</div>
        {#if filteredRecents.length === 0}
          <p class="empty">No matches</p>
        {/if}
        {#each filteredRecents as { item } (item.path)}
          <button type="button" class="row" onclick={() => onOpenPath(item.path)}>
            <span class="icon" aria-hidden="true">🗂</span>
            <span class="row-labels">
              <span class="row-name">{item.name}</span>
              <span class="row-path">{item.path}</span>
            </span>
          </button>
        {/each}
      {/if}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 40%);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
    z-index: 100;
  }

  .picker {
    width: min(560px, 90vw);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--overlay);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    box-shadow: 0 16px 48px rgb(0 0 0 / 35%);
    overflow: hidden;
  }

  .search {
    font: inherit;
    font-size: 15px;
    padding: var(--space-3) var(--space-4);
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--text);
    outline: none;
  }

  .search::placeholder {
    color: var(--text-faint);
  }

  .results {
    overflow-y: auto;
    padding: var(--space-2);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    border-radius: var(--radius-control);
    color: var(--text);
    text-align: left;
    font: inherit;
    cursor: pointer;
  }

  .row:hover {
    background: var(--raised);
  }

  .row.action {
    color: var(--accent);
  }

  .icon {
    flex-shrink: 0;
  }

  .row-labels {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .row-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-path {
    font-size: 11px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .section-label {
    padding: var(--space-2) var(--space-3) 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
  }

  .empty {
    padding: var(--space-2) var(--space-3);
    color: var(--text-faint);
    font-size: 12px;
  }
</style>
