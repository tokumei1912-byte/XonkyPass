<script lang="ts">
  import { hideCurrentWindow } from '../lib/window';

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') void hideCurrentWindow();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main>
  <input type="search" placeholder="Поиск появится на этапе 3" disabled />
  <p class="muted"><kbd>Esc</kbd> — закрыть</p>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    height: 100%;
    padding: var(--space-4);
    border: 1px solid var(--border);
  }

  input {
    width: 100%;
    padding: var(--space-3) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
