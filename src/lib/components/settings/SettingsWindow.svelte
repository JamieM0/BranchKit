<script lang="ts">
	import { settingsWindow, type SettingsSection } from "$lib/stores/settingsWindow.svelte";
	import { appSettings } from "$lib/stores/appSettings.svelte";
	import { settings } from "$lib/stores/settings.svelte";
	import { theme } from "$lib/stores/theme.svelte";
	import { github } from "$lib/stores/github.svelte";
	import * as ipc from "$lib/ipc";
	import type { CredentialInfo, SshAgentStatus, SshKeyInfo } from "$lib/types";
	import RevealSection from "./RevealSection.svelte";
	import SettingField from "./SettingField.svelte";

	/** The Settings window — DESIGN_SPEC.md §13: single window (Cmd+,), left nav with six sections,
	 * dynamic reveal, instant persistence, one-line descriptions. */

	const SECTIONS: { id: SettingsSection; label: string }[] = [
		{ id: "general", label: "General" },
		{ id: "appearance", label: "Appearance" },
		{ id: "git", label: "Git" },
		{ id: "credentials", label: "Credentials" },
		{ id: "ai", label: "AI" },
		{ id: "integrations", label: "Integrations" },
	];

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			e.preventDefault();
			settingsWindow.dismiss();
		}
	}

	// --- Credentials section state ---
	let credentials: CredentialInfo[] = $state([]);
	let sshStatus: SshAgentStatus | null = $state(null);
	let generatedKey: SshKeyInfo | null = $state(null);
	let showKeygenForm = $state(false);
	let keygenPassphrase = $state("");
	let keygenAcknowledgedEmptyRisk = $state(false);
	let keygenError = $state("");
	let keygenBusy = $state(false);

	async function loadCredentialsSection() {
		try {
			credentials = await ipc.listCredentials();
		} catch {
			credentials = [];
		}
		try {
			sshStatus = await ipc.getSshAgentStatus();
		} catch {
			sshStatus = null;
		}
		try {
			generatedKey = await ipc.getGeneratedSshKey();
		} catch {
			generatedKey = null;
		}
	}

	$effect(() => {
		if (settingsWindow.open && settingsWindow.section === "credentials") {
			void loadCredentialsSection();
		}
	});

	async function removeCred(c: CredentialInfo) {
		await ipc.removeCredential(c.host, c.username);
		credentials = credentials.filter((x) => !(x.host === c.host && x.username === c.username));
	}

	async function generateKey() {
		if (keygenPassphrase === "" && !keygenAcknowledgedEmptyRisk) return;
		keygenBusy = true;
		keygenError = "";
		try {
			generatedKey = await ipc.generateSshKey(keygenPassphrase);
			showKeygenForm = false;
			keygenPassphrase = "";
		} catch (e) {
			keygenError = e instanceof Error ? e.message : String(e);
		} finally {
			keygenBusy = false;
		}
	}

	async function copyPubkey() {
		if (!generatedKey) return;
		await navigator.clipboard?.writeText(generatedKey.publicKey);
	}

	// --- Integrations section ---
	$effect(() => {
		if (settingsWindow.open && settingsWindow.section === "integrations" && !github.checked) {
			void github.checkConnection();
		}
	});
</script>

{#if settingsWindow.open}
	<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
	<div class="scrim" onclick={() => settingsWindow.dismiss()} onkeydown={handleKeydown} role="presentation">
		<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
		<div class="window" role="dialog" aria-modal="true" aria-label="Settings" onclick={(e) => e.stopPropagation()}>
			<nav class="nav">
				{#each SECTIONS as s (s.id)}
					<button
						type="button"
						class="nav-item"
						class:active={settingsWindow.section === s.id}
						onclick={() => (settingsWindow.section = s.id)}
					>
						{s.label}
					</button>
				{/each}
			</nav>

			<div class="content">
				<button type="button" class="close" aria-label="Close settings" onclick={() => settingsWindow.dismiss()}>
					✕
				</button>

				{#if settingsWindow.section === "general"}
					<h2>General</h2>
					<SettingField label="Auto-fetch interval" description="How often BranchKit quietly fetches while the window is focused.">
						<select
							value={appSettings.current.general.autoFetchIntervalMinutes}
							onchange={(e) =>
								appSettings.update((d) => {
									d.general.autoFetchIntervalMinutes = Number(e.currentTarget.value);
								})}
						>
							<option value={0}>Off</option>
							<option value={1}>Every minute</option>
							<option value={5}>Every 5 minutes</option>
							<option value={15}>Every 15 minutes</option>
						</select>
					</SettingField>
					<SettingField label="Open last repos on launch" description="Reopen the tabs you had open the last time you quit BranchKit.">
						<input
							type="checkbox"
							checked={appSettings.current.general.openLastReposOnLaunch}
							onchange={(e) =>
								appSettings.update((d) => {
									d.general.openLastReposOnLaunch = e.currentTarget.checked;
								})}
						/>
					</SettingField>
					<SettingField label="Default clone directory" description="Where new clones are suggested to land; still changeable per clone.">
						<div class="row">
							<input
								type="text"
								class="text-input"
								placeholder="Not set — pick each time"
								value={appSettings.current.general.defaultCloneDir ?? ""}
								onchange={(e) =>
									appSettings.update((d) => {
										d.general.defaultCloneDir = e.currentTarget.value || null;
									})}
							/>
							<button
								type="button"
								class="secondary"
								onclick={async () => {
									const dir = await ipc.pickFolder("Default clone directory");
									if (dir) appSettings.update((d) => (d.general.defaultCloneDir = dir));
								}}
							>
								Choose…
							</button>
						</div>
					</SettingField>
				{:else if settingsWindow.section === "appearance"}
					<h2>Appearance</h2>
					<SettingField label="Theme" description="System follows your OS; Dark and Light are fixed.">
						<select value={theme.setting} onchange={(e) => theme.set(e.currentTarget.value as "system" | "dark" | "light")}>
							<option value="system">System</option>
							<option value="dark">Dark</option>
							<option value="light">Light</option>
						</select>
					</SettingField>
					<SettingField label="Graph density" description="Comfortable is the default row height; Compact fits more rows on screen.">
						<select
							value={appSettings.current.appearance.graphDensity}
							onchange={(e) =>
								appSettings.update((d) => {
									d.appearance.graphDensity = e.currentTarget.value as "comfortable" | "compact";
								})}
						>
							<option value="comfortable">Comfortable</option>
							<option value="compact">Compact</option>
						</select>
					</SettingField>
					<SettingField label="Date style" description="Relative shows “3h ago”; Absolute shows the calendar date.">
						<select
							value={appSettings.current.appearance.dateStyle}
							onchange={(e) =>
								appSettings.update((d) => {
									d.appearance.dateStyle = e.currentTarget.value as "relative" | "absolute";
								})}
						>
							<option value="relative">Relative</option>
							<option value="absolute">Absolute</option>
						</select>
					</SettingField>
					<SettingField label="Show avatars" description="Show author avatars on commit nodes instead of plain dots.">
						<input
							type="checkbox"
							checked={appSettings.current.appearance.showAvatars}
							onchange={(e) =>
								appSettings.update((d) => {
									d.appearance.showAvatars = e.currentTarget.checked;
								})}
						/>
					</SettingField>
				{:else if settingsWindow.section === "git"}
					<h2>Git</h2>
					<SettingField label="Default pull mode" description="What the toolbar's primary Pull button does with one click.">
						<select
							value={appSettings.current.git.defaultPullMode}
							onchange={(e) =>
								appSettings.update((d) => {
									d.git.defaultPullMode = e.currentTarget.value as "ff" | "rebase" | "merge";
								})}
						>
							<option value="ff">Fast-forward if possible</option>
							<option value="rebase">Rebase</option>
							<option value="merge">Merge</option>
						</select>
					</SettingField>
					<SettingField label="Push tags with commits" description="Include tags automatically whenever you push.">
						<input
							type="checkbox"
							checked={appSettings.current.git.pushTagsWithCommits}
							onchange={(e) =>
								appSettings.update((d) => {
									d.git.pushTagsWithCommits = e.currentTarget.checked;
								})}
						/>
					</SettingField>
					<SettingField label="Prune on fetch" description="Remove local tracking references for branches deleted on the remote.">
						<input
							type="checkbox"
							checked={appSettings.current.git.pruneOnFetch}
							onchange={(e) =>
								appSettings.update((d) => {
									d.git.pruneOnFetch = e.currentTarget.checked;
								})}
						/>
					</SettingField>
					<SettingField label="Combine tracking branches" description="Show a tracked remote branch nested under its local branch instead of twice.">
						<input type="checkbox" checked={settings.combineTrackingBranches} onchange={(e) => settings.setCombineTracking(e.currentTarget.checked)} />
					</SettingField>
					<SettingField label="Commit summary guide length" description="Where the composer's character countdown warns you a summary is getting long.">
						<input
							type="number"
							class="number-input"
							min="20"
							max="200"
							value={appSettings.current.git.commitSummaryGuideLength}
							onchange={(e) =>
								appSettings.update((d) => {
									d.git.commitSummaryGuideLength = Math.max(20, Math.min(200, Number(e.currentTarget.value) || 72));
								})}
						/>
					</SettingField>
				{:else if settingsWindow.section === "credentials"}
					<h2>Credentials</h2>
					<section class="sub">
						<h3>Stored HTTPS credentials</h3>
						<p class="hint">Saved automatically the first time a push/pull/fetch needs them.</p>
						{#if credentials.length === 0}
							<p class="empty">No stored credentials yet.</p>
						{:else}
							<ul class="cred-list">
								{#each credentials as c (c.host + ':' + c.username)}
									<li class="cred-row">
										<span class="cred-host">{c.host}</span>
										<span class="cred-user">{c.username}</span>
										<span class="cred-used">last used {new Date(c.lastUsedAt * 1000).toLocaleDateString()}</span>
										<button type="button" class="link-btn danger" onclick={() => void removeCred(c)}>Remove</button>
									</li>
								{/each}
							</ul>
						{/if}
					</section>

					<section class="sub">
						<h3>SSH</h3>
						<p class="hint">
							{#if sshStatus?.agentRunning}
								ssh-agent is running with {sshStatus.identities.length} loaded {sshStatus.identities.length === 1 ? "identity" : "identities"}.
							{:else}
								No ssh-agent detected — BranchKit delegates to your own agent and never manages passphrases.
							{/if}
						</p>

						{#if generatedKey}
							<div class="pubkey-box">
								<code class="pubkey">{generatedKey.publicKey}</code>
								<button type="button" class="secondary" onclick={copyPubkey}>Copy</button>
							</div>
							<p class="hint">
								Add this key at
								<button type="button" class="link-btn" onclick={() => void ipc.openInBrowser('https://github.com/settings/keys')}>
									github.com/settings/keys
								</button>
							</p>
						{:else if showKeygenForm}
							<label class="field">
								<span>Passphrase (optional)</span>
								<input type="password" bind:value={keygenPassphrase} placeholder="Leave blank for no passphrase" />
							</label>
							{#if keygenPassphrase === ""}
								<label class="ack">
									<input type="checkbox" bind:checked={keygenAcknowledgedEmptyRisk} />
									<span>I understand this key will have no passphrase</span>
								</label>
							{/if}
							{#if keygenError}<p class="error">{keygenError}</p>{/if}
							<div class="actions">
								<button type="button" class="secondary" onclick={() => (showKeygenForm = false)}>Cancel</button>
								<button
									type="button"
									class="primary"
									disabled={keygenBusy || (keygenPassphrase === "" && !keygenAcknowledgedEmptyRisk)}
									onclick={generateKey}
								>
									{keygenBusy ? "Generating…" : "Generate key"}
								</button>
							</div>
						{:else}
							<button type="button" class="secondary" onclick={() => (showKeygenForm = true)}>Generate new key…</button>
						{/if}
					</section>
				{:else if settingsWindow.section === "ai"}
					<h2>AI</h2>
					<SettingField label="Enable AI commit messages" description="Generate a summary and description from your staged changes.">
						<input
							type="checkbox"
							checked={appSettings.current.ai.enabled}
							onchange={(e) =>
								appSettings.update((d) => {
									d.ai.enabled = e.currentTarget.checked;
								})}
						/>
					</SettingField>

					<RevealSection open={appSettings.current.ai.enabled}>
						<div class="ai-provider-radio">
							<label>
								<input
									type="radio"
									name="ai-provider"
									checked={appSettings.current.ai.provider === "local"}
									onchange={() => appSettings.update((d) => (d.ai.provider = "local"))}
								/>
								Local
							</label>
							<label>
								<input
									type="radio"
									name="ai-provider"
									checked={appSettings.current.ai.provider === "ollama"}
									onchange={() => appSettings.update((d) => (d.ai.provider = "ollama"))}
								/>
								Ollama
							</label>
							<label>
								<input
									type="radio"
									name="ai-provider"
									checked={appSettings.current.ai.provider === "remote"}
									onchange={() => appSettings.update((d) => (d.ai.provider = "remote"))}
								/>
								Remote API
							</label>
						</div>

						<RevealSection open={appSettings.current.ai.provider === "local"}>
							<div class="model-card">
								<div class="model-card-head">
									<span>Gemma 3 1B (Q4, ~800 MB)</span>
									<span class="badge-muted">Not downloaded</span>
								</div>
								<p class="hint">Runs entirely on this machine. Downloading and running the local model lands in a later prompt.</p>
							</div>
						</RevealSection>

						<RevealSection open={appSettings.current.ai.provider === "ollama"}>
							<SettingField label="Ollama URL" description="Where your local Ollama server is listening.">
								<input
									type="text"
									class="text-input"
									value={appSettings.current.ai.ollamaBaseUrl}
									onchange={(e) =>
										appSettings.update((d) => {
											d.ai.ollamaBaseUrl = e.currentTarget.value;
										})}
								/>
							</SettingField>
							<SettingField label="Model" description="Populated from /api/tags once the provider is wired up.">
								<input
									type="text"
									class="text-input"
									placeholder="e.g. llama3.1"
									value={appSettings.current.ai.ollamaModel ?? ""}
									onchange={(e) =>
										appSettings.update((d) => {
											d.ai.ollamaModel = e.currentTarget.value || null;
										})}
								/>
							</SettingField>
						</RevealSection>

						<RevealSection open={appSettings.current.ai.provider === "remote"}>
							<SettingField label="API format" description="OpenAI-compatible or Anthropic's Messages API shape.">
								<select
									value={appSettings.current.ai.remoteFormat}
									onchange={(e) =>
										appSettings.update((d) => {
											d.ai.remoteFormat = e.currentTarget.value as "openAi" | "anthropic";
										})}
								>
									<option value="openAi">OpenAI-compatible</option>
									<option value="anthropic">Anthropic</option>
								</select>
							</SettingField>
							<SettingField label="Base URL" description="The API endpoint to send requests to.">
								<input
									type="text"
									class="text-input"
									value={appSettings.current.ai.remoteBaseUrl}
									onchange={(e) =>
										appSettings.update((d) => {
											d.ai.remoteBaseUrl = e.currentTarget.value;
										})}
								/>
							</SettingField>
							<SettingField label="API key" description="Stored in your OS keychain, never written to a config file.">
								<input type="password" class="text-input" placeholder="Provider activation lands in a later prompt" disabled />
							</SettingField>
							<SettingField label="Model name" description="Passed as the model field on every request.">
								<input
									type="text"
									class="text-input"
									value={appSettings.current.ai.remoteModel}
									onchange={(e) =>
										appSettings.update((d) => {
											d.ai.remoteModel = e.currentTarget.value;
										})}
								/>
							</SettingField>
						</RevealSection>

						<SettingField label="Style" description="Plain sentences, or Conventional Commits formatting.">
							<select
								value={appSettings.current.ai.style}
								onchange={(e) =>
									appSettings.update((d) => {
										d.ai.style = e.currentTarget.value as "plain" | "conventional";
									})}
							>
								<option value="plain">Plain</option>
								<option value="conventional">Conventional Commits</option>
							</select>
						</SettingField>
						<SettingField label="Max diff size" description="Diffs larger than this are truncated before being sent.">
							<input
								type="range"
								min="1"
								max="64"
								value={appSettings.current.ai.maxDiffSizeKb}
								oninput={(e) =>
									appSettings.update((d) => {
										d.ai.maxDiffSizeKb = Number(e.currentTarget.value);
									})}
							/>
							<span class="range-value">{appSettings.current.ai.maxDiffSizeKb} KB</span>
						</SettingField>
					</RevealSection>
				{:else if settingsWindow.section === "integrations"}
					<h2>Integrations</h2>
					<section class="sub">
						<h3>GitHub</h3>
						{#if github.connected && github.user}
							<div class="gh-connected">
								<img class="gh-avatar" src={github.user.avatarUrl} alt="" />
								<span>Signed in as <strong>{github.user.login}</strong></span>
								<button type="button" class="secondary" onclick={() => void github.signOut()}>Sign out</button>
							</div>
						{:else if github.deviceCode}
							<div class="device-flow">
								<p class="device-code">{github.deviceCode.userCode}</p>
								<div class="actions">
									<button type="button" class="secondary" onclick={() => navigator.clipboard?.writeText(github.deviceCode!.userCode)}>
										Copy code
									</button>
									<button type="button" class="primary" onclick={() => void ipc.openInBrowser(github.deviceCode!.verificationUri)}>
										Open github.com/login/device
									</button>
								</div>
								<p class="hint">{github.polling ? "Waiting for you to enter the code on GitHub…" : "Starting…"}</p>
								<button type="button" class="link-btn" onclick={() => github.cancelSignIn()}>Cancel</button>
							</div>
						{:else}
							{#if github.signInError}<p class="error">{github.signInError}</p>{/if}
							<button type="button" class="primary" onclick={() => void github.beginSignIn()}>Sign in with GitHub</button>
							<p class="hint">Unlocks pull requests, CI status, and PR creation for GitHub-hosted repos.</p>
						{/if}
					</section>
				{/if}
			</div>
		</div>
	</div>
{/if}

<style>
	.scrim {
		position: fixed;
		inset: 0;
		background: rgb(0 0 0 / 45%);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 200;
	}

	.window {
		width: min(720px, 92vw);
		height: min(560px, 88vh);
		display: flex;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius-card);
		box-shadow: 0 24px 64px rgb(0 0 0 / 45%);
		overflow: hidden;
	}

	.nav {
		width: 160px;
		flex-shrink: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-3) var(--space-2);
		background: var(--raised);
		border-right: 1px solid var(--border);
	}

	.nav-item {
		text-align: left;
		padding: var(--space-2) var(--space-3);
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-muted);
		font: inherit;
		font-size: 13px;
		cursor: pointer;
	}

	.nav-item:hover {
		background: var(--overlay);
		color: var(--text);
	}

	.nav-item.active {
		background: var(--overlay);
		color: var(--text);
		font-weight: 600;
	}

	.content {
		position: relative;
		flex: 1;
		min-width: 0;
		overflow-y: auto;
		padding: var(--space-5);
	}

	.close {
		position: absolute;
		top: var(--space-3);
		right: var(--space-3);
		width: 24px;
		height: 24px;
		border: none;
		border-radius: var(--radius-control);
		background: none;
		color: var(--text-muted);
		cursor: pointer;
	}

	.close:hover {
		background: var(--raised);
		color: var(--text);
	}

	h2 {
		margin: 0 0 var(--space-3);
		font-size: 15px;
		font-weight: 600;
		color: var(--text);
	}

	h3 {
		margin: 0 0 2px;
		font-size: 12px;
		font-weight: 700;
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.sub {
		margin-top: var(--space-4);
		padding-top: var(--space-3);
		border-top: 1px solid var(--border-soft);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.hint {
		margin: 0;
		font-size: 11px;
		color: var(--text-faint);
	}

	.empty {
		margin: 0;
		font-size: 12px;
		color: var(--text-faint);
	}

	select,
	.text-input,
	.number-input {
		font: inherit;
		font-size: 12px;
		padding: 4px var(--space-2);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		color: var(--text);
	}

	.number-input {
		width: 64px;
	}

	.row {
		display: flex;
		gap: var(--space-2);
	}

	.row .text-input {
		flex: 1;
		min-width: 0;
	}

	button {
		font: inherit;
		font-size: 12px;
		padding: 5px var(--space-3);
		border-radius: var(--radius-control);
		border: 1px solid transparent;
		cursor: pointer;
	}

	button.secondary {
		background: var(--raised);
		color: var(--text);
		border-color: var(--border);
	}

	button.primary {
		background: var(--accent);
		color: var(--bg);
		font-weight: 600;
	}

	button.primary:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.link-btn {
		background: none;
		border: none;
		color: var(--info);
		padding: 0;
		text-decoration: underline;
		cursor: pointer;
	}

	.link-btn.danger {
		color: var(--danger);
	}

	.cred-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.cred-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 12px;
		padding: 4px 0;
		border-bottom: 1px solid var(--border-soft);
	}

	.cred-host {
		font-weight: 600;
		color: var(--text);
	}

	.cred-user {
		color: var(--text-muted);
	}

	.cred-used {
		margin-left: auto;
		color: var(--text-faint);
		font-size: 11px;
	}

	.pubkey-box {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.pubkey {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: 11px;
		padding: var(--space-2);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 2px;
		font-size: 12px;
		color: var(--text-muted);
	}

	.field input {
		font: inherit;
		padding: 4px var(--space-2);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
		color: var(--text);
	}

	.ack {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		font-size: 11px;
		color: var(--warn);
	}

	.error {
		font-size: 11px;
		color: var(--danger);
	}

	.actions {
		display: flex;
		gap: var(--space-2);
	}

	.ai-provider-radio {
		display: flex;
		gap: var(--space-4);
		font-size: 12px;
		padding: var(--space-2) 0;
	}

	.ai-provider-radio label {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.model-card {
		display: flex;
		flex-direction: column;
		gap: 4px;
		padding: var(--space-3);
		background: var(--raised);
		border: 1px solid var(--border);
		border-radius: var(--radius-control);
	}

	.model-card-head {
		display: flex;
		justify-content: space-between;
		font-size: 12px;
		color: var(--text);
	}

	.badge-muted {
		color: var(--text-faint);
		font-size: 11px;
	}

	.range-value {
		margin-left: var(--space-2);
		font-size: 11px;
		color: var(--text-muted);
	}

	.gh-connected {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: 12px;
	}

	.gh-avatar {
		width: 24px;
		height: 24px;
		border-radius: var(--radius-pill);
	}

	.device-flow {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		align-items: flex-start;
	}

	.device-code {
		margin: 0;
		font-family: var(--font-mono);
		font-size: 28px;
		font-weight: 700;
		letter-spacing: 0.1em;
		color: var(--text);
	}
</style>
