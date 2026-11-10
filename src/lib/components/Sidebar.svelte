<script lang="ts">
  import { onMount } from 'svelte';
  import { store } from '../store.svelte';
  import NoteList from './NoteList.svelte';
  import Icon from './Icon.svelte';

  let search: HTMLInputElement;

  onMount(() => {
    store.focusSearch = () => {
      search.focus();
      search.select();
    };
  });

  const timeFormat = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });

  const syncLabel = $derived.by(() => {
    const s = store.syncStatus;
    switch (s.state) {
      case 'off':
        return 'Sync off';
      case 'syncing':
        return 'Syncing…';
      case 'offline':
        return 'Offline';
      case 'error':
        return 'Sync error';
      case 'idle':
        return s.lastSyncedAt ? `Synced ${timeFormat.format(new Date(s.lastSyncedAt))}` : 'Synced';
    }
  });
</script>

<aside class="sidebar">
  <header>
    <input
      bind:this={search}
      bind:value={store.query}
      oninput={() => store.refresh()}
      type="search"
      placeholder={store.filter === 'trash' ? 'Search (notes only)' : 'Search'}
      aria-label="Search notes"
      disabled={store.filter === 'trash'}
    />
    <button class="new" onclick={() => store.create()} title="New note (Ctrl+N)" aria-label="New note">
      <Icon name="plus" />
    </button>
  </header>

  <nav class="tabs">
    <button class:on={store.filter === 'notes'} onclick={() => store.setFilter('notes')}>Notes</button>
    <button class:on={store.filter === 'trash'} onclick={() => store.setFilter('trash')}>Trash</button>
  </nav>

  <div class="scroll">
    <NoteList />
  </div>

  <footer>
    {#if store.filter === 'trash' && store.list.length > 0}
      <button class="link" onclick={() => store.emptyTrash()}>Empty trash</button>
    {:else}
      <button
        class="link sync {store.syncStatus.state}"
        onclick={() => (store.syncStatus.state === 'off' ? (store.settingsOpen = true) : store.syncNow())}
        title={store.syncStatus.message ?? (store.syncStatus.state === 'off' ? 'Set up sync' : 'Sync now')}
      >
        <span class="dot" aria-hidden="true"></span>{syncLabel}
      </button>
    {/if}
    <span class="spacer"></span>
    <button class="link icon" onclick={() => (store.settingsOpen = true)} title="Settings" aria-label="Settings">
      <Icon name="settings" size={16} />
    </button>
  </footer>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--surface);
    border-right: 1px solid var(--border);
  }
  header {
    display: flex;
    gap: 8px;
    padding: 12px 12px 8px;
  }
  input[type='search'] {
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    color: inherit;
    font: inherit;
  }
  input[type='search']:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .new {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 34px;
    border: 0;
    border-radius: 8px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
  }
  .tabs {
    display: flex;
    gap: 4px;
    padding: 0 12px 6px;
  }
  .tabs button {
    border: 0;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: 0.9em;
    padding: 4px 8px;
    border-radius: 6px;
    cursor: pointer;
  }
  .tabs button.on {
    color: var(--text);
    background: var(--surface-2);
    font-weight: 600;
  }
  .scroll {
    flex: 1;
    overflow: auto;
  }
  footer {
    display: flex;
    align-items: center;
    padding: 6px 12px;
    border-top: 1px solid var(--border);
    font-size: 0.85em;
  }
  .spacer {
    flex: 1;
  }
  .link {
    border: 0;
    background: none;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
    padding: 4px;
  }
  .link:hover {
    color: var(--text);
  }
  .icon {
    display: inline-flex;
  }
  .sync {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    white-space: nowrap;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
  }
  .sync.idle .dot {
    background: var(--ok);
  }
  .sync.syncing .dot {
    background: var(--accent);
  }
  .sync.offline .dot {
    background: var(--warn);
  }
  .sync.error .dot {
    background: var(--danger);
  }

  /* Touch screens: 44px tap targets. */
  @media (pointer: coarse) {
    input[type='search'] {
      min-height: 44px;
    }
    .new {
      width: 44px;
    }
    .tabs button {
      min-height: 40px;
      padding: 4px 14px;
    }
    footer {
      padding: 2px 8px;
    }
    .link {
      min-height: 44px;
      padding: 4px 8px;
    }
    .icon {
      min-width: 44px;
      align-items: center;
      justify-content: center;
    }
  }
</style>
