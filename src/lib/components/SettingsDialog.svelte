<script lang="ts">
  import { api, type Theme } from '../api';
  import { store } from '../store.svelte';
  import Icon from './Icon.svelte';

  let dialog: HTMLDialogElement;
  let syncUrl = $state('');
  let syncToken = $state('');
  let showToken = $state(false);
  let saving = $state(false);
  let saved = $state(false);

  $effect(() => {
    if (store.settingsOpen) void open();
    else if (dialog?.open) dialog.close();
  });

  async function open() {
    try {
      const s = await api.getSettings();
      syncUrl = s.syncUrl ?? '';
      syncToken = s.syncToken ?? '';
    } catch (e) {
      store.fail(e);
    }
    showToken = false;
    saved = false;
    if (!dialog.open) dialog.showModal();
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    saved = await store.saveSync(syncUrl, syncToken);
    saving = false;
  }

  async function disconnect() {
    syncUrl = '';
    syncToken = '';
    saved = await store.saveSync('', '');
  }

  const themes: { value: Theme; label: string }[] = [
    { value: 'system', label: 'System' },
    { value: 'light', label: 'Light' },
    { value: 'dark', label: 'Dark' },
  ];

  const statusText = $derived.by(() => {
    const s = store.syncStatus;
    if (s.state === 'off') return 'Sync is off.';
    if (s.state === 'syncing') return 'Syncing…';
    if (s.state === 'idle')
      return s.lastSyncedAt ? `Last synced ${new Date(s.lastSyncedAt).toLocaleString()}.` : 'Connected.';
    return s.message ?? (s.state === 'offline' ? 'Offline. Will retry.' : 'Sync failed.');
  });
</script>

<dialog bind:this={dialog} onclose={() => (store.settingsOpen = false)} aria-labelledby="settings-title">
  <header>
    <h2 id="settings-title">Settings</h2>
    <button class="icon" onclick={() => (store.settingsOpen = false)} aria-label="Close">
      <Icon name="close" />
    </button>
  </header>

  <section>
    <h3>Theme</h3>
    <div class="segmented" role="radiogroup" aria-label="Theme">
      {#each themes as t (t.value)}
        <button
          role="radio"
          aria-checked={store.theme === t.value}
          class:on={store.theme === t.value}
          onclick={() => store.setTheme(t.value)}>{t.label}</button
        >
      {/each}
    </div>
  </section>

  <section>
    <h3>Sync</h3>
    <p class="hint">
      Sync uses your own free Cloudflare server. Run <code>npm run setup</code> in the
      <code>worker</code> folder once; it prints the URL and token to paste here on every device.
    </p>
    <form onsubmit={save}>
      <label>
        Sync URL
        <input
          bind:value={syncUrl}
          oninput={() => (saved = false)}
          type="url"
          placeholder="https://ferrite-notes-sync.you.workers.dev"
          autocomplete="off"
          spellcheck="false"
        />
      </label>
      <label>
        Sync token
        <span class="token">
          <input
            bind:value={syncToken}
            oninput={() => (saved = false)}
            type={showToken ? 'text' : 'password'}
            autocomplete="off"
            spellcheck="false"
          />
          <button type="button" class="small" onclick={() => (showToken = !showToken)}>
            {showToken ? 'Hide' : 'Show'}
          </button>
        </span>
      </label>
      <p class="status {store.syncStatus.state}" role="status">{saved ? 'Saved. ' : ''}{statusText}</p>
      <div class="actions">
        {#if store.syncStatus.state !== 'off'}
          <button type="button" class="secondary" onclick={disconnect}>Turn off sync</button>
          <button type="button" class="secondary" onclick={() => store.syncNow()}>
            <Icon name="sync" size={15} /> Sync now
          </button>
        {/if}
        <button type="submit" class="primary" disabled={saving}>Save</button>
      </div>
    </form>
  </section>
</dialog>

<style>
  dialog {
    width: min(480px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.25);
  }
  dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 12px 4px 20px;
  }
  h2 {
    margin: 0;
    font-size: 1.1em;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 0.8em;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }
  section {
    padding: 12px 20px 16px;
  }
  section + section {
    border-top: 1px solid var(--border);
  }
  .hint {
    margin: 0 0 12px;
    font-size: 0.88em;
    color: var(--text-muted);
    line-height: 1.5;
  }
  code {
    font-family: var(--font-mono);
    font-size: 0.92em;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.88em;
  }
  input {
    width: 100%;
    min-width: 0;
    padding: 7px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    color: inherit;
    font: inherit;
    font-size: 15px;
  }
  input:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .token {
    display: flex;
    gap: 6px;
  }
  button {
    font: inherit;
    cursor: pointer;
    border-radius: 8px;
  }
  .icon {
    display: inline-flex;
    padding: 6px;
    border: 0;
    background: none;
    color: var(--text-muted);
  }
  .icon:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .small,
  .secondary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
  }
  .primary {
    padding: 6px 16px;
    border: 0;
    background: var(--accent);
    color: var(--on-accent);
  }
  .primary:disabled {
    opacity: 0.6;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }
  .status {
    margin: 0;
    font-size: 0.85em;
    color: var(--text-muted);
  }
  .status.error {
    color: var(--danger);
  }
  .segmented {
    display: inline-flex;
    padding: 2px;
    border-radius: 8px;
    background: var(--surface-2);
  }
  .segmented button {
    padding: 4px 14px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--text-muted);
  }
  .segmented button.on {
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.1);
  }
</style>
