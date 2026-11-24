<script lang="ts" generics="T extends string">
  interface Props {
    options: readonly { value: T; label: string }[];
    value: T;
    label: string;
    onchange?: (value: T) => void;
  }

  let { options, value = $bindable(), label, onchange }: Props = $props();

  function select(next: T) {
    value = next;
    onchange?.(next);
  }

  // Стрелки переключают вкладки, как в стандартном tablist.
  function onkeydown(e: KeyboardEvent) {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    const index = options.findIndex((o) => o.value === value);
    const step = e.key === 'ArrowRight' ? 1 : -1;
    const next = options[(index + step + options.length) % options.length];
    if (next) {
      select(next.value);
      (e.currentTarget as HTMLElement)
        .querySelector<HTMLElement>(`[data-value="${CSS.escape(next.value)}"]`)
        ?.focus();
    }
  }
</script>

<div class="seg" role="tablist" aria-label={label} tabindex="-1" {onkeydown}>
  {#each options as option (option.value)}
    <button
      type="button"
      role="tab"
      class:on={option.value === value}
      aria-selected={option.value === value}
      tabindex={option.value === value ? 0 : -1}
      data-value={option.value}
      onclick={() => select(option.value)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: flex;
    flex: none;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-lg);
    background: var(--hover);
    outline: 0;
  }

  button {
    flex: 1;
    height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-2);
    font-size: 13px;
    white-space: nowrap;
    cursor: pointer;
  }

  button:hover {
    color: var(--text);
  }

  button.on {
    background: var(--popup);
    color: var(--text);
    font-weight: 600;
    box-shadow: var(--shadow-sm);
  }
</style>
