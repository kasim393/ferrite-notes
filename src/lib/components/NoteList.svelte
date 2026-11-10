<script lang="ts">
  import { untrack } from 'svelte';
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

  /*
   * Render the list a page at a time: 5,000 rows of DOM cost ~400 ms at startup
   * and ~200 MB in WebView2. More rows are added as the end scrolls into view.
   */
  const PAGE = 150;
  let shown = $state(PAGE);
  let sentinel = $state<HTMLElement>();
  const visible = $derived(store.list.slice(0, shown));

  $effect.pre(() => {
    void store.list; // a new list (filter, search, refresh) starts at one page again…
    shown = Math.max(PAGE, untrack(() => activeIndex()) + 1); // …but keeps the open note in view
  });

  function activeIndex() {
    const id = store.current?.id;
    return id ? store.list.findIndex((n) => n.id === id) : -1;
  }

  $effect(() => {
    if (!sentinel) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) shown += PAGE;
      },
      { rootMargin: '400px' },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  });

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
    {#each visible as note (note.id)}
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
          {#if note.snippet || note.tasksTotal}
            <span class="snippet">
              {#if note.tasksTotal}<span
                  class="tasks"
                  class:all-done={note.tasksDone === note.tasksTotal}
                  aria-label="{note.tasksDone} of {note.tasksTotal} tasks done"
                  >{note.tasksDone} of {note.tasksTotal}</span
                >{/if}{@html snippetHtml(note.snippet)}
            </span>
          {/if}
        </button>
      </li>
    {/each}
    {#if shown < store.list.length}
      <li bind:this={sentinel} class="more" aria-hidden="true"></li>
    {/if}
  </ul>
{/if}

<style>
  .more {
    height: 1px;
  }
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
    border-radius: var(--radius);
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item {
    border: 1px solid transparent;
  }
  .item.active {
    background: var(--selected);
    border-color: var(--selected-border);
    color: var(--selected-text);
  }
  .item.active .time,
  .item.active .snippet,
  .item.active .pin {
    color: var(--selected-muted);
  }
  .tasks {
    display: inline-block;
    margin-right: 6px;
    padding: 0 6px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
    font-size: 0.92em;
  }
  .tasks.all-done {
    background: none;
    color: inherit;
    padding: 0;
  }
  .item.active .tasks {
    background: color-mix(in srgb, currentColor 18%, transparent);
    color: inherit;
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
