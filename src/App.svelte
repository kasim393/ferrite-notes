<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { store } from './lib/store.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Editor from './lib/components/Editor.svelte';
  import Preview from './lib/components/Preview.svelte';

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

  function onkeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    if (mod && !e.shiftKey && key === 'n') {
      e.preventDefault();
      void store.create();
    } else if (mod && !e.shiftKey && key === 'f') {
      e.preventDefault();
      store.focusSearch();
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

<div class="app" class:has-note={store.current != null}>
  <Sidebar />

  <main class="pane">
    {#if store.current}
      {@const note = store.current}
      <div class="toolbar">
        <button class="icon back" onclick={() => store.close()} aria-label="Back to list">‹</button>
        <span class="status" class:error={store.saveStatus === 'error'}>{statusText[store.saveStatus]}</span>
        <span class="spacer"></span>
        {#if note.deleted}
          <button onclick={() => store.restore()}>Restore</button>
        {:else}
          <button class:on={store.preview} onclick={() => store.togglePreview()} title="Preview (Ctrl+P)">
            {store.preview ? 'Edit' : 'Preview'}
          </button>
          <button class:on={note.pinned} onclick={() => store.togglePin()} title="Pin (Ctrl+Shift+P)">
            {note.pinned ? 'Unpin' : 'Pin'}
          </button>
          <button onclick={() => store.trash()} title="Move to trash (Ctrl+Backspace outside the editor)">
            Trash
          </button>
        {/if}
      </div>

      {#if store.preview || note.deleted}
        <Preview />
      {:else}
        {#key `${note.id}:${store.docVersion}`}
          <Editor body={note.body} />
        {/key}
      {/if}
    {:else}
      <div class="placeholder">
        <p>Select a note or press <kbd>Ctrl</kbd>+<kbd>N</kbd></p>
      </div>
    {/if}
  </main>

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
    grid-template-columns: minmax(220px, 300px) 1fr;
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
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }
  .toolbar button {
    border: 1px solid transparent;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.88em;
    padding: 4px 10px;
    border-radius: 6px;
    cursor: pointer;
  }
  .toolbar button:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .toolbar button.on {
    color: var(--accent);
    border-color: var(--border);
  }
  .back {
    display: none;
    font-size: 1.4em !important;
    line-height: 1;
  }
  .status {
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

  /* Phone: list screen → note screen. */
  @media (max-width: 700px) {
    .app {
      grid-template-columns: 1fr;
    }
    .app.has-note :global(.sidebar),
    .app:not(.has-note) .pane {
      display: none;
    }
    .back {
      display: inline-block;
    }
  }
</style>
