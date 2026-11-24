<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorText, type AppInfo } from '../lib/api';
  import { hideCurrentWindow } from '../lib/window';

  let info = $state<AppInfo | null>(null);
  let error = $state<string | null>(null);

  onMount(() => {
    api
      .appInfo()
      .then((value) => (info = value))
      .catch((e: unknown) => (error = errorText(e)));
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') void hideCurrentWindow();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main>
  <header data-tauri-drag-region>
    <div class="logo" aria-hidden="true">X</div>
    <div class="title" data-tauri-drag-region>
      <h1>XonkyPass</h1>
      <span class="muted">
        {#if info}версия {info.version}{:else if error}ошибка IPC{:else}…{/if}
      </span>
    </div>
  </header>

  <section class="placeholder">
    <p>Здесь будет список паролей и заметок.</p>
    <p class="muted">Этап 0: каркас приложения, трей и сборка установщика.</p>
    {#if error}
      <p class="error">{error}</p>
    {/if}
  </section>

  <footer class="muted"><kbd>Esc</kbd> — спрятать</footer>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    height: 100%;
    border: 1px solid var(--border);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4);
    border-bottom: 1px solid var(--border);
  }

  .logo {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg);
    font-family: var(--font-display);
    font-weight: 700;
    pointer-events: none;
  }

  .title {
    display: flex;
    flex-direction: column;
  }

  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
    pointer-events: none;
  }

  .placeholder {
    flex: 1;
    padding: var(--space-6) var(--space-4);
  }

  .placeholder p {
    margin: 0 0 var(--space-2);
  }

  footer {
    padding: var(--space-3) var(--space-4);
    border-top: 1px solid var(--border);
  }

  .muted {
    color: var(--text-muted);
    font-size: 12px;
  }

  .error {
    color: #e5484d;
    user-select: text;
  }
</style>
