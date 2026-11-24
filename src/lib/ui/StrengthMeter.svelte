<script lang="ts">
  import type { Strength } from '../api';
  import { strengthLabel } from '../format';

  interface Props {
    /** `null` — пароль пустой или ещё не оценён. */
    strength: Strength | null;
    /** Пояснение справа: длина, время подбора. */
    detail?: string;
  }

  let { strength, detail }: Props = $props();

  const info = $derived(strength ? strengthLabel(strength) : null);
  const bars = $derived(strength ? Math.max(strength.score, 1) : 0);
</script>

<div class="strength" aria-live="polite">
  <div class="meter {info?.tone ?? ''}" aria-hidden="true">
    {#each [1, 2, 3, 4] as bar (bar)}
      <i class:filled={bar <= bars}></i>
    {/each}
  </div>
  {#if info}
    <span class="label {info.tone}">{info.label}</span>
  {/if}
  {#if detail}
    <span class="detail">{detail}</span>
  {/if}
</div>

<style>
  .strength {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 18px;
    font-size: var(--text-sm);
  }

  .meter {
    display: flex;
    flex: 1;
    gap: 4px;
  }

  .meter i {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--border-strong);
    transition: background var(--duration);
  }

  .meter.danger i.filled {
    background: var(--danger);
  }

  .meter.warn i.filled {
    background: var(--warn);
  }

  .meter.ok i.filled {
    background: var(--ok);
  }

  .label {
    font-weight: 600;
  }

  .label.danger {
    color: var(--danger);
  }

  .label.warn {
    color: var(--warn);
  }

  .label.ok {
    color: var(--ok);
  }

  .detail {
    color: var(--text-2);
  }
</style>
