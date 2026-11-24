<script lang="ts" generics="T extends string | number">
  import { tick } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    options: readonly { value: T; label: string }[];
    value: T | undefined;
    /** Для `<label for>`. */
    id?: string;
    /** Подпись для экранного диктора, если рядом нет `<label for>`. */
    label?: string;
    /** id элемента с подписью — когда подпись не `<label>`. */
    labelledby?: string;
    disabled?: boolean;
    onchange?: (value: T) => void;
  }

  let {
    options,
    value = $bindable(),
    id,
    label,
    labelledby,
    disabled = false,
    onchange,
  }: Props = $props();

  let open = $state(false);
  let active = $state(0);
  let button: HTMLButtonElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();
  /** Положение списка: под полем или над ним, у левого или правого края поля. */
  let place = $state('');
  const listId = $props.id();

  const selected = $derived(options.findIndex((option) => option.value === value));
  const current = $derived(options[selected]);

  const ITEM_HEIGHT = 34;

  async function show() {
    if (disabled || open || !button) return;
    const rect = button.getBoundingClientRect();
    const height = Math.min(options.length * ITEM_HEIGHT + 10, 320);
    const spaceBelow = window.innerHeight - rect.bottom - 8;
    const above = height > spaceBelow && rect.top - 8 > spaceBelow;
    const vertical = above
      ? `bottom: ${window.innerHeight - rect.top + 4}px`
      : `top: ${rect.bottom + 4}px`;
    // Поля стоят у правого края строк настроек — список выравниваем по той же стороне.
    const right = rect.left + rect.width / 2 > window.innerWidth / 2;
    const horizontal = right
      ? `right: ${window.innerWidth - rect.right}px`
      : `left: ${rect.left}px`;
    place = `${vertical}; ${horizontal}; min-width: ${rect.width}px; max-height: ${height}px`;
    active = Math.max(selected, 0);
    open = true;
    await tick();
    scrollToActive();
  }

  function close(focus = true) {
    if (!open) return;
    open = false;
    if (focus) button?.focus();
  }

  function choose(index: number) {
    const option = options[index];
    close();
    if (option && option.value !== value) {
      value = option.value;
      onchange?.(option.value);
    }
  }

  function scrollToActive() {
    list?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  function move(to: number) {
    active = Math.min(Math.max(to, 0), options.length - 1);
    scrollToActive();
  }

  /** Буква — к следующему пункту на эту букву, как в обычном списке Windows. */
  function typeahead(key: string): boolean {
    if (key.length !== 1 || !/\p{L}|\p{N}/u.test(key)) return false;
    const letter = key.toLowerCase();
    for (let step = 1; step <= options.length; step++) {
      const index = (active + step) % options.length;
      if (options[index]?.label.toLowerCase().startsWith(letter)) {
        move(index);
        return true;
      }
    }
    return false;
  }

  function onkeydown(e: KeyboardEvent) {
    if (disabled) return;
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' ', 'F4'].includes(e.key)) {
        e.preventDefault();
        void show();
      }
      return;
    }
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        move(active + 1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        move(active - 1);
        break;
      case 'Home':
      case 'PageUp':
        e.preventDefault();
        move(0);
        break;
      case 'End':
      case 'PageDown':
        e.preventDefault();
        move(options.length - 1);
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        choose(active);
        break;
      case 'Escape':
        // Только закрыть список, а не уйти с экрана.
        e.preventDefault();
        e.stopPropagation();
        close();
        break;
      case 'Tab':
        close(false);
        break;
      default:
        if (typeahead(e.key)) e.preventDefault();
    }
  }

  function onpointerdown(e: PointerEvent) {
    const target = e.target as Node;
    if (open && !button?.contains(target) && !list?.contains(target)) close(false);
  }

  function onscroll(e: Event) {
    if (open && !list?.contains(e.target as Node)) close(false);
  }
</script>

<svelte:window
  {onpointerdown}
  onscrollcapture={onscroll}
  onresize={() => close(false)}
  onblur={() => close(false)}
/>

<button
  bind:this={button}
  type="button"
  class="select"
  class:open
  {id}
  {disabled}
  role="combobox"
  aria-label={label}
  aria-labelledby={labelledby}
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={listId}
  aria-activedescendant={open ? `${listId}-${active}` : undefined}
  onclick={() => (open ? close() : show())}
  {onkeydown}
>
  <span class="value">{current?.label ?? ''}</span>
  <Icon name="chevron-down" size={14} />
</button>

{#if open}
  <div bind:this={list} class="popup list" id={listId} role="listbox" style={place} tabindex="-1">
    {#each options as option, i (option.value)}
      <!-- Фокус остаётся на поле: пункты выбираются стрелками там же. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        id="{listId}-{i}"
        class="mi option"
        class:active={i === active}
        class:selected={i === selected}
        role="option"
        tabindex="-1"
        aria-selected={i === selected}
        data-index={i}
        onmousedown={(e) => e.preventDefault()}
        onclick={() => choose(i)}
        onmouseenter={() => (active = i)}
      >
        {option.label}
      </div>
    {/each}
  </div>
{/if}

<style>
  .select {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 120px;
    height: var(--control);
    padding: 0 10px 0 12px;
    border: 1px solid var(--border);
    border-bottom-color: var(--border-strong);
    border-radius: var(--radius);
    outline-offset: 1px;
    background: var(--surface);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }

  .select:hover {
    background: color-mix(in srgb, var(--surface), var(--text) 4%);
  }

  .select:active,
  .select.open {
    background: color-mix(in srgb, var(--surface), var(--text) 2%);
  }

  .select:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .value {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .select :global(svg) {
    flex: none;
    color: var(--text-2);
  }

  .list {
    position: fixed;
    z-index: 40;
    max-width: calc(100vw - 16px);
    padding: 4px;
    overflow-y: auto;
    outline: 0;
    animation: drop var(--duration) var(--ease);
  }

  .option {
    position: relative;
    height: 32px;
    margin: 1px 0;
    padding: 0 12px;
    white-space: nowrap;
    user-select: none;
  }

  /* Выбранный пункт — полоска акцента слева, как в списках Windows 11. */
  .option.selected {
    background: var(--hover);
  }

  .option.selected::before {
    position: absolute;
    top: 50%;
    left: 0;
    width: 3px;
    height: 16px;
    border-radius: 2px;
    background: var(--accent);
    content: '';
    transform: translateY(-50%);
  }

  .option.active:not(.selected) {
    background: var(--hover);
  }

  .option.active.selected {
    background: color-mix(in srgb, var(--hover), var(--text) 4%);
  }

  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
