<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { Look } from '../api';
  import { store } from '../store.svelte';
  import Icon from './Icon.svelte';

  /*
   * Our own title bar (the window has `decorations: false`), drawn by the theme.
   * The bar itself drags the window; double-click maximizes (Tauri's drag region).
   * macOS gets the three dots on the left; the other themes keep Windows' order on the right.
   */
  const win = getCurrentWindow();

  const titles: Record<Look, string> = {
    macos: 'Ferrite Notes',
    gaming: 'FERRITE NOTES',
    terminal: 'ferrite@notes: ~',
    eink: 'Ferrite Notes',
    retro: 'FERRITE NOTES  MODEL FN-01',
  };

  let maximized = $state(false);
  $effect(() => {
    let unlisten: (() => void) | undefined;
    const update = () => win.isMaximized().then((m) => (maximized = m)).catch(() => {});
    update();
    win.onResized(update).then((fn) => (unlisten = fn)).catch(() => {});
    return () => unlisten?.();
  });

  const minimize = () => void win.minimize().catch((e) => store.fail(e));
  const toggleMaximize = () => void win.toggleMaximize().catch((e) => store.fail(e));
  const close = () => void win.close().catch((e) => store.fail(e));
</script>

<header class="titlebar" class:mac={store.look === 'macos'} data-tauri-drag-region>
  {#if store.look === 'macos'}
    <div class="dots">
      <button class="dot close" onclick={close} aria-label="Close" title="Close"></button>
      <button class="dot min" onclick={minimize} aria-label="Minimize" title="Minimize"></button>
      <button class="dot max" onclick={toggleMaximize} aria-label={maximized ? 'Restore' : 'Maximize'} title={maximized ? 'Restore' : 'Maximize'}></button>
    </div>
  {/if}

  <!-- Retro only (themes.css): a lamp showing save and sync activity. -->
  <span
    class="lamp"
    class:busy={store.saveStatus === 'saving' || store.syncStatus.state === 'syncing'}
    class:error={store.saveStatus === 'error' || store.syncStatus.state === 'error'}
    data-tauri-drag-region>TX/RX</span
  >

  <span class="title" data-tauri-drag-region>{titles[store.look]}</span>

  {#if store.look !== 'macos'}
    <div class="controls">
      <!-- Retro shows the printed .key-label instead of the icon (themes.css). -->
      <button onclick={minimize} aria-label="Minimize" title="Minimize">
        <Icon name="minimize" size={16} /><span class="key-label">MIN</span>
      </button>
      <button onclick={toggleMaximize} aria-label={maximized ? 'Restore' : 'Maximize'} title={maximized ? 'Restore' : 'Maximize'}>
        <Icon name={maximized ? 'restoreWindow' : 'maximize'} size={14} /><span class="key-label">{maximized ? 'RST' : 'MAX'}</span>
      </button>
      <button class="close" onclick={close} aria-label="Close" title="Close">
        <Icon name="close" size={16} /><span class="key-label">OFF</span>
      </button>
    </div>
  {/if}
</header>

<style>
  .titlebar {
    position: relative;
    display: flex;
    align-items: center;
    height: 34px;
    flex-shrink: 0;
    background: var(--titlebar-bg);
    color: var(--titlebar-text);
    border-bottom: 1px solid var(--titlebar-border);
    user-select: none;
  }
  .title {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    font-size: 0.8em;
    white-space: pre;
    pointer-events: none;
    letter-spacing: var(--titlebar-tracking);
    font-weight: var(--titlebar-weight);
  }
  .dots {
    display: flex;
    gap: 8px;
    padding: 0 14px;
  }
  .dot {
    width: 12px;
    height: 12px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    cursor: pointer;
  }
  .dot.close {
    background: #ff5f57;
  }
  .dot.min {
    background: #febc2e;
  }
  .dot.max {
    background: #28c840;
  }
  .dot:hover {
    filter: brightness(0.85);
  }
  .controls {
    display: flex;
    align-items: center;
    margin-left: auto;
    height: 100%;
  }
  .controls button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .controls button:hover {
    background: var(--surface-2);
  }
  .controls .close:hover {
    background: #e81123;
    color: #fff;
  }
</style>
