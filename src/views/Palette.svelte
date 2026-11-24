<script lang="ts">
  import Icon from '../lib/ui/Icon.svelte';
  import { hideCurrentWindow } from '../lib/window';

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') void hideCurrentWindow();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main data-tauri-drag-region>
  <div class="inp search">
    <Icon name="search" />
    <input
      type="search"
      placeholder="Быстрый поиск появится на этапе 3"
      aria-label="Поиск"
      disabled
    />
  </div>
  <p class="muted small"><kbd>Esc</kbd> — закрыть</p>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    height: 100%;
    padding: var(--space-4);
  }

  .search {
    height: 44px;
    padding-left: 12px;
    font-size: 16px;
  }

  .search :global(svg) {
    flex: none;
    color: var(--text-2);
  }
</style>
