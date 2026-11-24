<script lang="ts">
  import { fly } from 'svelte/transition';
  import Icon from './Icon.svelte';
  import { toasts } from './overlays.svelte';

  interface Props {
    /** Отступ снизу: над нижней панелью экрана. */
    bottom?: number;
  }

  let { bottom = 12 }: Props = $props();

  let now = $state(Date.now());
  const counting = $derived(toasts.items.some((t) => t.countdown));

  $effect(() => {
    if (!counting) return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 200);
    return () => clearInterval(timer);
  });

  function secondsLeft(endsAt: number): number {
    return Math.max(0, Math.ceil((endsAt - now) / 1000));
  }
</script>

<div class="toasts" style:bottom="{bottom}px" role="status" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div class="toast popup {toast.tone}" transition:fly={{ y: 12, duration: 160 }}>
      <Icon name={toast.icon} size={toast.countdown ? 20 : 18} class="icon" />
      <div class="text">
        <div class="title">{toast.title}</div>
        {#if toast.countdown}
          <div class="detail">Буфер очистится через {secondsLeft(toast.countdown.endsAt)} с</div>
        {:else if toast.detail}
          <div class="detail">{toast.detail}</div>
        {/if}
      </div>
      {#if toast.action}
        {@const action = toast.action}
        <button
          type="button"
          class="btn ghost sm"
          class:primary={!toast.countdown}
          onclick={() => {
            toasts.dismiss(toast.id);
            void action.run();
          }}
        >
          {action.label}
        </button>
      {:else}
        <button
          type="button"
          class="ib sm"
          aria-label="Закрыть"
          onclick={() => toasts.dismiss(toast.id)}
        >
          <Icon name="x" size={14} />
        </button>
      {/if}
      {#if toast.countdown}
        <div
          class="progress"
          style:width="{Math.max(0, (toast.countdown.endsAt - now) / toast.countdown.total) * 100}%"
        ></div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: absolute;
    right: 12px;
    left: 12px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: none;
  }

  .toast {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 8px 6px 8px 12px;
    overflow: hidden;
    pointer-events: auto;
  }

  .toast :global(.icon) {
    color: var(--text-2);
  }

  .toast.success :global(.icon) {
    color: var(--ok);
  }

  .toast.error :global(.icon) {
    color: var(--danger);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .detail {
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  .primary {
    color: var(--accent);
    font-weight: 600;
  }

  .progress {
    position: absolute;
    bottom: 0;
    left: 0;
    height: 3px;
    background: var(--accent);
    transition: width 200ms linear;
  }
</style>
