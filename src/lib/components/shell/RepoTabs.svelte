<script lang="ts">
  import { repos, type RepoTab } from "$lib/stores/repo.svelte";

  let { onPick }: { onPick: () => void } = $props();

  let dragIndex: number | null = $state(null);
  let dragOverIndex: number | null = $state(null);

  function onDragStart(e: DragEvent, index: number) {
    dragIndex = index;
    e.dataTransfer?.setData("text/plain", String(index));
    if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    dragOverIndex = index;
  }

  function onDrop(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragIndex !== null) repos.reorder(dragIndex, index);
    dragIndex = null;
    dragOverIndex = null;
  }

  function onDragEnd() {
    dragIndex = null;
    dragOverIndex = null;
  }

  function onMiddleClick(e: MouseEvent, tab: RepoTab) {
    if (e.button !== 1) return;
    e.preventDefault();
    void repos.close(tab.id);
  }

  function onClose(e: MouseEvent, tab: RepoTab) {
    e.stopPropagation();
    void repos.close(tab.id);
  }
</script>

<div class="tabs" role="tablist">
  {#each repos.tabs as tab, index (tab.id)}
    <div
      role="tab"
      tabindex="0"
      aria-selected={repos.activeId === tab.id}
      class="tab"
      class:active={repos.activeId === tab.id}
      class:drag-over={dragOverIndex === index && dragIndex !== index}
      draggable={!tab.cloneProgress}
      onclick={() => repos.switchTo(tab.id)}
      onkeydown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          repos.switchTo(tab.id);
        }
      }}
      onmousedown={(e) => onMiddleClick(e, tab)}
      ondragstart={(e) => onDragStart(e, index)}
      ondragover={(e) => onDragOver(e, index)}
      ondrop={(e) => onDrop(e, index)}
      ondragend={onDragEnd}
    >
      <span class="dot-slot">
        {#if tab.busy}
          <span class="spinner" aria-hidden="true"></span>
        {:else}
          <span class="dot" aria-hidden="true"></span>
        {/if}
      </span>
      <span class="labels">
        <span class="name">{tab.name}</span>
        {#if tab.cloneProgress}
          <span class="branch">{tab.cloneProgress.phase}{tab.cloneProgress.percent !== null ? ` ${tab.cloneProgress.percent}%` : "…"}</span>
        {:else if tab.detached}
          <span class="branch">detached</span>
        {:else if tab.branch}
          <span class="branch">{tab.branch}</span>
        {/if}
      </span>
      {#if !tab.cloneProgress}
        <button
          type="button"
          class="close"
          aria-label="Close {tab.name}"
          onclick={(e) => onClose(e, tab)}
        >
          ✕
        </button>
      {/if}
    </div>
  {/each}
  <button type="button" class="new-tab" onclick={onPick} aria-label="Open or clone a repository (⌘T)">
    +
  </button>
</div>

<style>
  .tabs {
    display: flex;
    align-items: stretch;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    padding: 0 var(--space-2);
    gap: 2px;
    overflow-x: auto;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-muted);
    cursor: pointer;
    font: inherit;
    white-space: nowrap;
    transition: background var(--motion-hover), color var(--motion-hover);
  }

  .tab:hover {
    background: var(--raised);
    color: var(--text);
  }

  .tab.active {
    color: var(--text);
    border-bottom-color: var(--accent);
  }

  .tab.drag-over {
    background: var(--overlay);
  }

  .dot-slot {
    display: inline-flex;
    width: 8px;
    height: 8px;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--text-faint);
  }

  .tab.active .dot {
    background: var(--accent);
  }

  .spinner {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-pill);
    border: 1.5px solid var(--border);
    border-top-color: var(--accent);
    animation: spin var(--motion-loop) linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .labels {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    line-height: 1.2;
  }

  .name {
    font-size: var(--font-size-ui);
  }

  .branch {
    font-size: 11px;
    color: var(--text-faint);
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: var(--radius-control);
    font-size: 10px;
    color: var(--text-faint);
    opacity: 0;
    transition: opacity var(--motion-hover), background var(--motion-hover);
  }

  .tab:hover .close {
    opacity: 1;
  }

  .close:hover {
    background: var(--overlay);
    color: var(--text);
  }

  .new-tab {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    align-self: center;
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 16px;
    cursor: pointer;
    border-radius: var(--radius-control);
  }

  .new-tab:hover {
    background: var(--raised);
    color: var(--text);
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
