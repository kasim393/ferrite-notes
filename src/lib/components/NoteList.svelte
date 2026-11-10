<script lang="ts">
  import { store } from '../store.svelte';

  /** Phones have no keyboard shortcut to suggest. */
  const coarse = window.matchMedia('(pointer: coarse)').matches;

  function escapeHtml(s: string) {
    return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
  }

  /** Search snippets mark matches with \u0002 … \u0003; escape first, then highlight. */
  function snippetHtml(s: string) {
    return escapeHtml(s).replaceAll('\u0002', '<mark>').replaceAll('\u0003', '</mark>');
  }

  const dayFormat = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' });
  const timeFormat = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });

  function when(ms: number) {
    const date = new Date(ms);
    return date.toDateString() === new Date().toDateString()
      ? timeFormat.format(date)
      : dayFormat.format(date);
  }
</script>

{#if store.list.length === 0}
  <p class="empty">
    {#if store.query.trim()}No matches.{:else if store.filter === 'trash'}Trash is empty.{:else}No notes yet. {coarse ? 'Tap + to add one.' : 'Press Ctrl+N.'}{/if}
  </p>
{:else}
  <ul class="list">
    {#each store.list as note (note.id)}
      <li>
        <button
          class="item"
          class:active={store.current?.id === note.id}
          onclick={() => store.open(note.id)}
        >
          <span class="row">
            <span class="title">{note.title || 'Untitled'}</span>
            {#if note.pinned && !note.deleted}<span class="pin" aria-label="Pinned">●</span>{/if}
            <span class="time">{when(note.updatedAt)}</span>
          </span>
          {#if note.snippet}
            <span class="snippet">{@html snippetHtml(note.snippet)}</span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .list {
    list-style: none;
    margin: 0;
    padding: 4px 8px 16px;
  }
  .item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item.active {
    background: var(--selected);
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .title {
    flex: 1;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pin {
    color: var(--accent);
    font-size: 0.7em;
  }
  .time {
    color: var(--text-muted);
    font-size: 0.8em;
    flex-shrink: 0;
  }
  .snippet {
    color: var(--text-muted);
    font-size: 0.88em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .snippet :global(mark) {
    background: var(--mark);
    color: inherit;
    border-radius: 2px;
  }
  .empty {
    color: var(--text-muted);
    padding: 16px 18px;
    margin: 0;
  }
</style>
