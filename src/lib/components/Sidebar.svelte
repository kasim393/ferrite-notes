<script lang="ts">
  import { onMount } from 'svelte';
  import type { Theme } from '../api';
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

  const nextTheme: Record<Theme, Theme> = { system: 'light', light: 'dark', dark: 'system' };
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
    {/if}
    <span class="spacer"></span>
    <button class="link" onclick={() => store.setTheme(nextTheme[store.theme])} title="Change theme">
      Theme: {store.theme}
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
</style>
