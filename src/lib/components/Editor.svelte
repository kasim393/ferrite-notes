<script lang="ts">
  import { onMount } from 'svelte';
  import { createEditor } from '../editor/setup';
  import { store } from '../store.svelte';

  // Mounted fresh per note (and per outside change), keyed by the parent.
  let { body }: { body: string } = $props();
  let host: HTMLDivElement;

  onMount(() => {
    const editor = createEditor(host, body, (doc) => store.edit(doc));
    store.focusEditor = () => editor.focus();
    return () => {
      store.focusEditor = () => {};
      editor.destroy();
    };
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .editor :global(.cm-editor) {
    flex: 1;
    min-width: 0;
  }
</style>
