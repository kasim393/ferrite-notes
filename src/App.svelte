<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { store } from './lib/store.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Editor from './lib/components/Editor.svelte';
  import Preview from './lib/components/Preview.svelte';
  import Icon from './lib/components/Icon.svelte';
  import SettingsDialog from './lib/components/SettingsDialog.svelte';

  const statusText = { saved: 'Saved', unsaved: 'Editing…', saving: 'Saving…', error: 'Not saved' };

  onMount(() => {
    const win = getCurrentWindow();
    let unlistenClose: (() => void) | undefined;

    (async () => {
      try {
        await store.init();
      } finally {
        // Window starts hidden; show it once there is something to show (no white flash).
        await win.show().catch(() => {});
      }
    })();

    // Save on close and when the window loses focus.
    win
      .onCloseRequested(async () => {
        await store.flush();
      })
      .then((fn) => (unlistenClose = fn))
      .catch((e) => store.fail(e));
    const onBlur = () => void store.flush();
    window.addEventListener('blur', onBlur);

    return () => {
      unlistenClose?.();
      window.removeEventListener('blur', onBlur);
    };
  });

  function inEditor() {
    return document.activeElement?.closest('.cm-editor') != null;
  }

  const sidebarLabel = $derived(`${store.sidebarCollapsed ? 'Show' : 'Hide'} sidebar (Ctrl+B)`);
  const narrowScreen = window.matchMedia('(max-width: 700px)');

  /** On narrow screens the list and note are separate screens, so there's nothing to collapse. */
  function toggleSidebar() {
    if (!narrowScreen.matches) store.setSidebarCollapsed(!store.sidebarCollapsed);
  }

  async function focusSearch() {
    if (store.sidebarCollapsed) {
      store.setSidebarCollapsed(false);
      await tick();
    }
    store.focusSearch();
  }

  function onkeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (mod && !e.shiftKey && key === 'n') {
      e.preventDefault();
      void store.create();
    } else if (mod && !e.shiftKey && key === 'f') {
      e.preventDefault();
      void focusSearch();
    } else if (mod && !e.shiftKey && key === 'b') {
      e.preventDefault();
      toggleSidebar();
    } else if (mod && e.shiftKey && key === 'p') {
      e.preventDefault();
      void store.togglePin();
    } else if (mod && !e.shiftKey && key === 'p') {
      e.preventDefault();
      void store.togglePreview();
    } else if (mod && key === 'backspace' && !inEditor()) {
      // In the editor Ctrl+Backspace keeps its usual "delete word" meaning.
      e.preventDefault();
      void store.trash();
    } else if (key === 'escape' && store.query) {
      store.query = '';
      void store.refresh();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="app" class:has-note={store.current != null} class:sidebar-collapsed={store.sidebarCollapsed}>
  <Sidebar />

  <main class="pane">
    <div class="toolbar">
      <button
        class="toggle-sidebar"
        onclick={toggleSidebar}
        aria-label={sidebarLabel}
        aria-expanded={!store.sidebarCollapsed}
        title={sidebarLabel}
      >
        <Icon name="sidebar" />
      </button>
      {#if store.current}
        {@const note = store.current}
        <button class="back" onclick={() => store.close()} aria-label="Back to list" title="Back to list">
          <Icon name="back" />
        </button>
        <span class="status" class:error={store.saveStatus === 'error'}>{statusText[store.saveStatus]}</span>
        <span class="spacer"></span>
        {#if note.deleted}
          <button onclick={() => store.restore()} aria-label="Restore" title="Restore">
            <Icon name="restore" />
          </button>
        {:else}
          {@const previewLabel = store.preview ? 'Edit (Ctrl+P)' : 'Preview (Ctrl+P)'}
          {@const pinLabel = note.pinned ? 'Unpin (Ctrl+Shift+P)' : 'Pin (Ctrl+Shift+P)'}
          <button onclick={() => store.togglePreview()} aria-label={previewLabel} title={previewLabel}>
            <Icon name={store.preview ? 'edit' : 'eye'} />
          </button>
          <button
            class:on={note.pinned}
            onclick={() => store.togglePin()}
            aria-label={pinLabel}
            aria-pressed={note.pinned}
            title={pinLabel}
          >
            <Icon name="pin" filled={note.pinned} />
          </button>
          <button
            onclick={() => store.trash()}
            aria-label="Move to trash"
            title="Move to trash (Ctrl+Backspace outside the editor)"
          >
            <Icon name="trash" />
          </button>
        {/if}
      {/if}
    </div>

    {#if store.current}
      {#if store.preview || store.current.deleted}
        <Preview />
      {:else}
        {#key `${store.current.id}:${store.docVersion}`}
          <Editor body={store.current.body} />
        {/key}
      {/if}
    {:else}
      <div class="placeholder">
        <p>Select a note or press <kbd>Ctrl</kbd>+<kbd>N</kbd></p>
      </div>
    {/if}
  </main>

  <SettingsDialog />

  {#if store.error}
    <div class="toast" role="alert">
      <span>{store.error}</span>
      <button onclick={() => (store.error = null)} aria-label="Dismiss">×</button>
    </div>
  {/if}
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: minmax(220px, 300px) minmax(0, 1fr);
    height: 100vh;
    height: 100dvh;
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 40px;
    flex-shrink: 0;
    padding: 0 6px;
    border-bottom: 1px solid var(--border);
  }
  .toolbar button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-muted);
    border-radius: 6px;
    cursor: pointer;
  }
  .toolbar button:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .toolbar button.on {
    color: var(--accent);
  }
  .toolbar .back {
    display: none;
  }
  .status {
    margin-left: 6px;
    color: var(--text-muted);
    font-size: 0.8em;
  }
  .status.error {
    color: var(--danger);
  }
  .spacer {
    flex: 1;
  }
  .placeholder {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }
  kbd {
    font-family: var(--font-mono);
    font-size: 0.85em;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 16px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: calc(100vw - 32px);
    padding: 10px 14px;
    border-radius: 8px;
    background: var(--danger);
    color: #fff;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.2);
  }
  .toast button {
    border: 0;
    background: none;
    color: inherit;
    font-size: 1.2em;
    cursor: pointer;
  }

  @media (min-width: 701px) {
    .app.sidebar-collapsed {
      grid-template-columns: minmax(0, 1fr);
    }
    .app.sidebar-collapsed :global(.sidebar) {
      display: none;
    }
  }

  /* Phone: list screen → note screen. */
  @media (max-width: 700px) {
    .app {
      grid-template-columns: minmax(0, 1fr);
    }
    .app.has-note :global(.sidebar),
    .app:not(.has-note) .pane {
      display: none;
    }
    .toolbar .back {
      display: inline-flex;
    }
    .toolbar .toggle-sidebar {
      display: none;
    }
  }
</style>
