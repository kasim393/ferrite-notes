<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { store } from './lib/store.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Editor from './lib/components/Editor.svelte';
  import Preview from './lib/components/Preview.svelte';
  import Icon from './lib/components/Icon.svelte';
  import SettingsDialog from './lib/components/SettingsDialog.svelte';

  const narrowScreen = window.matchMedia('(max-width: 700px)');

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
    // Phones send the app to the background without a blur or close.
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') void store.flush();
    };
    document.addEventListener('visibilitychange', onVisibility);
    const onNarrow = () => (narrow = narrowScreen.matches);
    narrowScreen.addEventListener('change', onNarrow);
    window.addEventListener('popstate', onPopState);

    return () => {
      unlistenClose?.();
      window.removeEventListener('blur', onBlur);
      document.removeEventListener('visibilitychange', onVisibility);
      narrowScreen.removeEventListener('change', onNarrow);
      window.removeEventListener('popstate', onPopState);
    };
  });

  /*
   * Back gesture / back button. Android's WebView goes back in history when it
   * can, so every screen the user can back out of (the note on a phone, then
   * Settings) gets a history entry, and popping one closes that screen.
   */
  let narrow = $state(narrowScreen.matches);
  let pushed = 0;
  let ignoreNextPop = false;

  const layers = $derived((narrow && store.current != null ? 1 : 0) + (store.settingsOpen ? 1 : 0));

  $effect(() => {
    const target = layers;
    if (target > pushed) {
      for (; pushed < target; pushed++) history.pushState({ ferrite: pushed + 1 }, '');
    } else if (target < pushed) {
      // Closed from the UI: drop the entries we no longer need.
      ignoreNextPop = true;
      history.go(target - pushed);
      pushed = target;
    }
  });

  function onPopState() {
    if (ignoreNextPop) {
      ignoreNextPop = false;
      return;
    }
    if (pushed === 0) return;
    pushed--;
    if (store.settingsOpen) store.settingsOpen = false;
    else if (store.current) void store.close();
  }

  function inEditor() {
    return document.activeElement?.closest('.cm-editor') != null;
  }

  const sidebarLabel = $derived(`${store.sidebarCollapsed ? 'Show' : 'Hide'} sidebar (Ctrl+B)`);

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
    /* Android draws edge to edge: keep clear of the status and gesture bars. */
    padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom)
      env(safe-area-inset-left);
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
    bottom: calc(16px + env(safe-area-inset-bottom));
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

  /* Touch screens: 44px tap targets. */
  @media (pointer: coarse) {
    .toolbar {
      height: 48px;
    }
    .toolbar button {
      width: 44px;
      height: 44px;
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
