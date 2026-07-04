<script lang="ts">
  import { theme } from "$lib/stores/theme.svelte";

  const surfaces = ["--bg", "--surface", "--raised", "--overlay", "--border", "--border-soft"];
  const text = ["--text", "--text-muted", "--text-faint"];
  const actions = ["--accent", "--accent-dim", "--info", "--warn", "--danger", "--ahead", "--behind"];
  const lanes = ["--lane-0", "--lane-1", "--lane-2", "--lane-3", "--lane-4", "--lane-5", "--lane-6", "--lane-7"];
  const statuses = [
    "--status-added",
    "--status-modified",
    "--status-deleted",
    "--status-renamed",
    "--status-conflicted",
    "--status-untracked",
  ];
  const spacing = ["--space-1", "--space-2", "--space-3", "--space-4", "--space-5"];
  const radii = ["--radius-control", "--radius-card", "--radius-pill"];
</script>

<svelte:head>
  <title>BranchKit — Tokens</title>
</svelte:head>

<div class="tokens-page">
  <header>
    <h1>Design tokens</h1>
    <div class="theme-switch">
      {#each ["dark", "light", "system"] as const as t}
        <button class:active={theme.setting === t} onclick={() => theme.set(t)}>{t}</button>
      {/each}
    </div>
  </header>

  <section>
    <h2>Surfaces</h2>
    <div class="swatches">
      {#each surfaces as name}
        <div class="swatch">
          <div class="chip" style="background: var({name}); border: 1px solid var(--border)"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Text</h2>
    <div class="swatches">
      {#each text as name}
        <div class="swatch">
          <div class="chip" style="background: var({name})"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Actions &amp; states</h2>
    <div class="swatches">
      {#each actions as name}
        <div class="swatch">
          <div class="chip" style="background: var({name})"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Graph lane palette</h2>
    <div class="swatches">
      {#each lanes as name}
        <div class="swatch">
          <div class="chip" style="background: var({name})"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>File status</h2>
    <div class="swatches">
      {#each statuses as name}
        <div class="swatch">
          <div class="chip" style="background: var({name})"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Spacing</h2>
    <div class="spacing-row">
      {#each spacing as name}
        <div class="spacing-item">
          <div class="spacing-bar" style="width: var({name}); height: var({name})"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Radius</h2>
    <div class="spacing-row">
      {#each radii as name}
        <div class="spacing-item">
          <div class="radius-box" style="border-radius: var({name})"></div>
          <code>{name}</code>
        </div>
      {/each}
    </div>
  </section>

  <section>
    <h2>Type</h2>
    <p style="font-family: var(--font-ui); font-size: var(--font-size-ui)">
      UI font, 13px — the quick brown fox jumps over the lazy dog
    </p>
    <p style="font-family: var(--font-mono); font-size: var(--font-size-mono)">
      Mono font, 12px — const fox = () =&gt; jump(lazyDog);
    </p>
  </section>
</div>

<style>
  .tokens-page {
    min-height: 100vh;
    padding: var(--space-5);
    background: var(--bg);
    color: var(--text);
    font-family: var(--font-ui);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-5);
  }

  h1 {
    color: var(--accent);
  }

  h2 {
    color: var(--text-muted);
    font-size: 1em;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-bottom: var(--space-3);
  }

  section {
    margin-bottom: var(--space-5);
  }

  .theme-switch button {
    font-family: inherit;
    background: var(--raised);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    padding: var(--space-1) var(--space-3);
    margin-left: var(--space-2);
    cursor: pointer;
  }

  .theme-switch button.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }

  .swatch {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
  }

  .chip {
    width: 64px;
    height: 64px;
    border-radius: var(--radius-card);
  }

  .spacing-row {
    display: flex;
    align-items: flex-end;
    gap: var(--space-4);
  }

  .spacing-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-1);
  }

  .spacing-bar {
    background: var(--accent);
  }

  .radius-box {
    width: 48px;
    height: 48px;
    background: var(--raised);
    border: 1px solid var(--border);
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--font-size-mono);
    color: var(--text-faint);
  }
</style>
