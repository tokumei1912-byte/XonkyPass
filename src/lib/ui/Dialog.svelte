<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade, scale } from 'svelte/transition';

  interface Props {
    title: string;
    onclose: () => void;
    children: Snippet;
    /** Кнопки внизу. Фокус получает элемент с `data-autofocus` или первая кнопка. */
    actions: Snippet;
  }

  let { title, onclose, children, actions }: Props = $props();

  let box: HTMLDivElement | undefined = $state();
  const titleId = $props.id();

  const FOCUSABLE =
    'button:not(:disabled), input:not(:disabled), select, textarea, [tabindex]:not([tabindex="-1"])';

  $effect(() => {
    if (!box) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const target =
      box.querySelector<HTMLElement>('[data-autofocus]') ??
      box.querySelector<HTMLElement>(FOCUSABLE);
    (target ?? box).focus();
    return () => previous?.focus();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onclose();
      return;
    }
    // Фокус не уходит из диалога по Tab.
    if (e.key === 'Tab' && box) {
      const items = [...box.querySelectorAll<HTMLElement>(FOCUSABLE)];
      const first = items[0];
      const last = items[items.length - 1];
      if (!first || !last) return;
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="scrim"
  transition:fade={{ duration: 120 }}
  onmousedown={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div
    bind:this={box}
    class="dialog popup"
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    tabindex="-1"
    transition:scale={{ start: 0.96, duration: 160 }}
    {onkeydown}
  >
    <div class="body">
      <h2 id={titleId}>{title}</h2>
      {@render children()}
    </div>
    <div class="actions">
      {@render actions()}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    padding: var(--space-6);
    background: var(--scrim);
  }

  .dialog {
    width: 100%;
    max-width: 360px;
    overflow: hidden;
    outline: 0;
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 20px 20px 18px;
  }

  h2 {
    font-size: 18px;
    font-weight: 600;
    line-height: 1.3;
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    padding: 16px 20px;
    border-top: 1px solid var(--border);
    background: var(--popup-2);
  }
</style>
