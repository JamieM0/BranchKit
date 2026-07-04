<script lang="ts">
  import { checkGitIdentity, setGitIdentity } from "$lib/ipc";
  import { onboarding } from "$lib/stores/onboarding.svelte";
  import { theme, type ThemeSetting } from "$lib/stores/theme.svelte";

  type Step = "theme" | "identity";
  let step: Step = $state("theme");
  let name = $state("");
  let email = $state("");
  let checking = $state(true);
  let saving = $state(false);
  let errorMessage = $state("");
  let nameInput: HTMLInputElement | undefined = $state();

  $effect(() => {
    if (step === "identity" && !checking) nameInput?.focus();
  });

  $effect(() => {
    if (step !== "identity") return;
    checking = true;
    checkGitIdentity().then((identity) => {
      if (identity.name && identity.email) {
        finish();
        return;
      }
      name = identity.name ?? "";
      email = identity.email ?? "";
      checking = false;
    });
  });

  function chooseTheme(setting: ThemeSetting) {
    theme.set(setting);
    step = "identity";
  }

  async function saveIdentity() {
    if (!name.trim() || !email.trim()) return;
    saving = true;
    errorMessage = "";
    try {
      await setGitIdentity(name.trim(), email.trim());
      finish();
    } catch (e) {
      errorMessage = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  function finish() {
    onboarding.finish();
  }
</script>

<div class="wizard">
  <div class="card">
    {#if step === "theme"}
      <h1>Welcome to BranchKit</h1>
      <p>Pick a look — you can change this later in Settings.</p>
      <div class="choices">
        <button type="button" onclick={() => chooseTheme("system")}>Match System</button>
        <button type="button" onclick={() => chooseTheme("dark")}>Dark</button>
        <button type="button" onclick={() => chooseTheme("light")}>Light</button>
      </div>
      <button type="button" class="skip" onclick={() => (step = "identity")}>Skip</button>
    {:else if step === "identity"}
      {#if checking}
        <p class="checking">Checking your git configuration…</p>
      {:else}
        <h1>Set your git identity</h1>
        <p>
          Every commit needs a name and email. This writes to your global git config, same as
          <code>git config --global user.name</code>.
        </p>
        <label class="field">
          <span>Name</span>
          <input bind:this={nameInput} bind:value={name} type="text" placeholder="Jane Doe" />
        </label>
        <label class="field">
          <span>Email</span>
          <input bind:value={email} type="email" placeholder="jane@example.com" />
        </label>
        {#if errorMessage}
          <p class="error">{errorMessage}</p>
        {/if}
        <div class="actions">
          <button type="button" class="skip" onclick={finish}>Skip</button>
          <button
            type="button"
            class="primary"
            disabled={saving || !name.trim() || !email.trim()}
            onclick={saveIdentity}
          >
            Continue
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .wizard {
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg);
  }

  .card {
    width: min(420px, 90vw);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-card);
    padding: var(--space-5);
    box-shadow: 0 16px 48px rgb(0 0 0 / 25%);
  }

  h1 {
    font-size: 18px;
    font-weight: 600;
    color: var(--text);
  }

  p {
    font-size: 13px;
    color: var(--text-muted);
    line-height: 1.5;
  }

  .checking {
    color: var(--text-faint);
  }

  code {
    font-family: var(--font-mono);
    font-size: 11px;
    background: var(--raised);
    padding: 1px 4px;
    border-radius: 4px;
  }

  .choices {
    display: flex;
    gap: var(--space-2);
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

  .error {
    font-size: 12px;
    color: var(--danger);
  }

  .actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: var(--space-2);
  }

  button {
    font: inherit;
    font-size: 13px;
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-control);
    border: 1px solid var(--border);
    background: var(--raised);
    color: var(--text);
    cursor: pointer;
    transition: background var(--motion-hover);
  }

  button:hover {
    background: var(--overlay);
  }

  .choices button {
    flex: 1;
  }

  .skip {
    background: transparent;
    border-color: transparent;
    color: var(--text-faint);
  }

  .primary {
    background: var(--accent);
    color: var(--bg);
    border-color: transparent;
    font-weight: 600;
  }

  .primary:hover {
    background: var(--accent-dim);
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
