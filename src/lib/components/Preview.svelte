<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { api } from '../api';
  import { store } from '../store.svelte';

  let html = $state('');

  $effect(() => {
    const id = store.current?.id;
    void store.docVersion;
    if (id) void load(id);
  });

  async function load(id: string) {
    try {
      await store.flush();
      html = await api.renderMarkdown(id);
    } catch (e) {
      store.fail(e);
    }
  }

  function onclick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target instanceof HTMLInputElement && target.dataset.line) {
      e.preventDefault();
      void store.toggleTask(Number(target.dataset.line));
      return;
    }
    // Links open in the system browser, never inside the app window.
    const link = target.closest('a');
    if (link) {
      e.preventDefault();
      const href = link.getAttribute('href') ?? '';
      if (/^(https?:|mailto:)/i.test(href)) openUrl(href).catch((err) => store.fail(err));
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="preview" {onclick}>
  <!-- Rendered and sanitized in Rust (pulldown-cmark + ammonia). -->
  {@html html}
</div>

<style>
  /* Web images (https only, see the CSP in tauri.conf.json) fit the note's width. */
  .preview :global(img) {
    max-width: 100%;
    height: auto;
    border-radius: var(--radius-sm);
  }
  .preview {
    flex: 1;
    overflow: auto;
    padding: 20px 28px 48px;
    line-height: 1.6;
    overflow-wrap: anywhere;
  }
  .preview :global(h1),
  .preview :global(h2),
  .preview :global(h3) {
    line-height: 1.25;
    margin: 1.2em 0 0.5em;
  }
  .preview :global(h1:first-child) {
    margin-top: 0;
  }
  .preview :global(a) {
    color: var(--accent);
  }
  .preview :global(code) {
    font-family: var(--font-mono);
    font-size: 0.92em;
    background: var(--surface-2);
    padding: 0.1em 0.35em;
    border-radius: 4px;
  }
  .preview :global(pre) {
    background: var(--surface-2);
    padding: 12px 14px;
    border-radius: var(--radius);
    overflow: auto;
  }
  .preview :global(pre code) {
    background: none;
    padding: 0;
  }
  .preview :global(blockquote) {
    margin: 0;
    padding-left: 14px;
    border-left: 3px solid var(--border);
    color: var(--text-muted);
  }
  .preview :global(table) {
    border-collapse: collapse;
  }
  .preview :global(th),
  .preview :global(td) {
    border: 1px solid var(--border);
    padding: 4px 10px;
  }
  .preview :global(li:has(> input[type='checkbox'])) {
    list-style: none;
    margin-left: -1.4em;
  }
  .preview :global(li:has(> input:checked)) {
    color: var(--text-muted);
  }
</style>
