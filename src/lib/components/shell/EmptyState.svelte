<script lang="ts">
  import { relativeTime } from "$lib/format";
  import { listRecents, pickFolder } from "$lib/ipc";
  import type { RecentRepo } from "$lib/types";

  let {
    onOpenPath,
    onRequestClone,
  }: { onOpenPath: (path: string) => void; onRequestClone: () => void } = $props();

  let recents: RecentRepo[] = $state([]);

  $effect(() => {
    listRecents().then((r) => {
      recents = r;
    });
  });

  async function handleOpen() {
    const path = await pickFolder("Open Repository");
    if (path) onOpenPath(path);
  }
</script>

<main class="empty">
  <div class="hero">
    <h1>BranchKit</h1>
    <p class="tagline">Open a repository, or clone one to get started.</p>
    <div class="actions">
      <button type="button" class="primary" onclick={handleOpen}>Open…</button>
      <button type="button" class="secondary" onclick={onRequestClone}>Clone…</button>
    </div>
  </div>

  {#if recents.length > 0}
    <div class="recents">
      <h2>Recent</h2>
      <div class="grid">
        {#each recents as recent (recent.path)}
          <button type="button" class="card" onclick={() => onOpenPath(recent.path)}>
            <span class="card-name">{recent.name}</span>
            <span class="card-path">{recent.path}</span>
            <span class="card-time">{relativeTime(recent.lastOpenedAt)}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</main>

<style>
  .empty {
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-5);
    background: var(--bg);
    color: var(--text);
    padding: var(--space-5);
  }

  .hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    text-align: center;
  }

  h1 {
    font-size: 28px;
    color: var(--accent);
    font-weight: 600;
  }

  .tagline {
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    gap: var(--space-3);
  }

  button {
    font: inherit;
    font-size: 13px;
    padding: var(--space-2) var(--space-5);
    border-radius: var(--radius-control);
    border: 1px solid transparent;
    cursor: pointer;
    transition: background var(--motion-hover);
  }

  button.primary {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }

  button.primary:hover {
    background: var(--accent-dim);
  }

  button.secondary {
    background: var(--raised);
    color: var(--text);
    border-color: var(--border);
  }

  button.secondary:hover {
    background: var(--overlay);
  }

  .recents {
    width: min(720px, 90vw);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .recents h2 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--space-3);
  }

  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--space-3);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    text-align: left;
    cursor: pointer;
    transition: border-color var(--motion-hover), background var(--motion-hover);
  }

  .card:hover {
    background: var(--raised);
    border-color: var(--accent-dim);
  }

  .card-name {
    font-size: 13px;
    color: var(--text);
    font-weight: 500;
  }

  .card-path {
    font-size: 11px;
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .card-time {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
  }
</style>
