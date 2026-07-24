<script lang="ts">
  import { pickFolder } from "$lib/ipc";
  import { repos } from "$lib/stores/repo.svelte";

  let { onDismiss }: { onDismiss: () => void } = $props();

  type Phase = "form" | "cloning" | "done" | "error";

  function suggestName(url: string): string {
    const trimmed = url.trim().replace(/\/+$/, "");
    const last = trimmed.split(/[/:]/).filter(Boolean).at(-1) ?? "";
    return last.replace(/\.git$/, "") || "repository";
  }

  let url = $state("");
  let parentDir: string | null = $state(null);
  let folderName = $state("");
  let lastSuggested = $state("");
  let phase: Phase = $state("form");
  let errorMessage = $state("");
  let clonedPath = $state("");
  let urlInput: HTMLInputElement | undefined = $state();

  $effect(() => {
    urlInput?.focus();
  });

  $effect(() => {
    const suggestion = suggestName(url);
    if (folderName === "" || folderName === lastSuggested) folderName = suggestion;
    lastSuggested = suggestion;
  });

  let destination = $derived(parentDir && folderName ? `${parentDir}/${folderName}` : null);

  // The store's clone() already owns progress; read the one pending tab it creates rather than
  // subscribing to the event stream a second time.
  let cloningTab = $derived(repos.tabs.find((t) => t.id.startsWith("pending:")));

  async function chooseParent() {
    const dir = await pickFolder("Clone into…");
    if (dir) parentDir = dir;
  }

  async function submit() {
    if (!url.trim() || !destination) return;
    phase = "cloning";
    try {
      const tab = await repos.clone(url.trim(), destination);
      clonedPath = tab.path;
      phase = "done";
    } catch (e) {
      errorMessage = e instanceof Error ? e.message : String(e);
      phase = "error";
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && phase !== "cloning") {
      e.preventDefault();
      onDismiss();
    }
  }
</script>

<div class="scrim" onkeydown={handleKeydown} role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Clone a repository">
    <h2>Clone a repository</h2>

    {#if phase === "form" || phase === "error"}
      <label class="field">
        <span>Repository URL</span>
        <input
          bind:this={urlInput}
          bind:value={url}
          type="text"
          placeholder="https://github.com/org/repo.git"
        />
      </label>

      <label class="field">
        <span>Clone into</span>
        <div class="row">
          <button type="button" class="secondary" onclick={chooseParent}>Choose Folder…</button>
          <input bind:value={folderName} type="text" placeholder="folder name" />
        </div>
        {#if destination}
          <p class="hint">{destination}</p>
        {:else}
          <p class="hint">Choose where to put it, then confirm the folder name.</p>
        {/if}
      </label>

      {#if phase === "error"}
        <p class="error">{errorMessage}</p>
      {/if}

      <div class="actions">
        <button type="button" class="secondary" onclick={onDismiss}>Cancel</button>
        <button type="button" class="primary" disabled={!url.trim() || !destination} onclick={submit}>
          Clone
        </button>
      </div>
    {:else if phase === "cloning"}
      <p class="hint">{destination}</p>
      <div class="progress-track">
        <div
          class="progress-fill"
          class:indeterminate={cloningTab?.cloneProgress?.percent == null}
          style:width={cloningTab?.cloneProgress?.percent != null ? `${cloningTab.cloneProgress.percent}%` : undefined}
        ></div>
      </div>
      <p class="phase">{cloningTab?.cloneProgress?.phase ?? "Starting…"}{cloningTab?.cloneProgress?.percent != null ? ` ${cloningTab.cloneProgress.percent}%` : ""}</p>
    {:else if phase === "done"}
      <p class="done">✓ Cloned into {clonedPath}</p>
      <div class="actions">
        <button type="button" class="primary" onclick={onDismiss}>Open now</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 40%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .dialog {
    width: min(480px, 90vw);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    background: var(--overlay);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    box-shadow: 0 16px 48px rgb(0 0 0 / 35%);
    padding: var(--space-5);
  }

  h2 {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    font-size: 12px;
    color: var(--text-muted);
  }

  .field input {
    font: inherit;
    font-size: 13px;
    padding: var(--space-2) var(--space-3);
    background: var(--raised);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    color: var(--text);
  }

  .field input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .row {
    display: flex;
    gap: var(--space-2);
  }

  .row input {
    flex: 1;
    min-width: 0;
  }

  .hint {
    font-size: 11px;
    color: var(--text-faint);
    overflow-wrap: anywhere;
  }

  .error {
    font-size: 12px;
    color: var(--danger);
  }

  .done {
    font-size: 13px;
    color: var(--accent);
    overflow-wrap: anywhere;
  }

  .phase {
    font-size: 12px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  button {
    font: inherit;
    font-size: 13px;
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-control);
    border: 1px solid transparent;
    cursor: pointer;
    transition: background var(--motion-hover);
  }

  button.secondary {
    background: var(--raised);
    color: var(--text);
    border-color: var(--border);
  }

  button.secondary:hover {
    background: var(--overlay);
  }

  button.primary {
    background: var(--accent);
    color: var(--bg);
    font-weight: 600;
  }

  button.primary:hover {
    background: var(--accent-dim);
  }

  button.primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .progress-track {
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--raised);
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: var(--radius-pill);
    transition: width var(--motion-panel);
  }

  .progress-fill.indeterminate {
    width: 40%;
    animation: indeterminate var(--motion-loop) ease-in-out infinite;
  }

  @keyframes indeterminate {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(250%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill.indeterminate {
      animation: none;
    }
  }
</style>
