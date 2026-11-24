<script lang="ts" module>
  export interface Suggestion {
    value: string;
    /** Сколько записей с этим значением (для тегов). */
    count: number;
  }
</script>

<script lang="ts">
  import { plural } from '../format';
  import Icon from './Icon.svelte';

  interface Props {
    values: string[];
    /** Подпись поля для экранного диктора. */
    label: string;
    id?: string;
    placeholder?: string;
    /** Показывать значения с префиксом, например `#` у тегов. */
    prefix?: string;
    /** Варианты для автодополнения. Без них — простой ввод списка. */
    suggestions?: readonly Suggestion[];
    /** Приведение введённого значения к единому виду; пустая строка — отбросить. */
    normalize?: (value: string) => string;
    /** Слово для пункта «Создать …». */
    noun?: string;
    /** Подсказки раскрываются вверх — поле у нижнего края окна. */
    above?: boolean;
  }

  let {
    values = $bindable(),
    label,
    id,
    placeholder,
    prefix = '',
    suggestions,
    normalize = (value) => value.trim(),
    noun = 'тег',
    above = false,
  }: Props = $props();

  let text = $state('');
  let input: HTMLInputElement | undefined = $state();
  let open = $state(false);
  let active = $state(0);
  const listId = $props.id();

  const taken = $derived(new Set(values.map((v) => v.toLowerCase())));
  const query = $derived(normalize(text).toLowerCase());

  type Option = { kind: 'existing'; value: string; count: number } | { kind: 'new'; value: string };

  const options = $derived.by((): Option[] => {
    if (!suggestions || !query) return [];
    const matches = suggestions
      .filter((s) => !taken.has(s.value.toLowerCase()) && s.value.toLowerCase().includes(query))
      .sort(
        (a, b) =>
          Number(b.value.toLowerCase().startsWith(query)) -
            Number(a.value.toLowerCase().startsWith(query)) || b.count - a.count,
      )
      .slice(0, 5)
      .map((s): Option => ({ kind: 'existing', ...s }));
    const exact = suggestions.some((s) => s.value.toLowerCase() === query);
    if (!exact && !taken.has(query)) matches.push({ kind: 'new', value: normalize(text) });
    return matches;
  });

  const showList = $derived(open && options.length > 0);

  function add(raw: string) {
    const value = normalize(raw);
    if (value && !taken.has(value.toLowerCase())) values = [...values, value];
    text = '';
    active = 0;
  }

  function remove(index: number) {
    values = values.filter((_, i) => i !== index);
    input?.focus();
  }

  function onkeydown(e: KeyboardEvent) {
    if (showList && (e.key === 'ArrowDown' || e.key === 'ArrowUp')) {
      e.preventDefault();
      const step = e.key === 'ArrowDown' ? 1 : -1;
      active = (active + step + options.length) % options.length;
      return;
    }
    if (e.key === 'Enter' || e.key === ',' || (e.key === 'Tab' && text.trim())) {
      if (!text.trim()) return;
      e.preventDefault();
      const option = showList ? options[active] : undefined;
      add(option ? option.value : text);
      return;
    }
    if (e.key === 'Escape' && showList) {
      e.preventDefault();
      e.stopPropagation();
      open = false;
      return;
    }
    if (e.key === 'Backspace' && !text && values.length > 0) {
      e.preventDefault();
      values = values.slice(0, -1);
    }
  }

  function oninput() {
    // Вставка списка через запятую сразу превращается в несколько значений.
    if (text.includes(',')) {
      const parts = text.split(',');
      const rest = parts.pop() ?? '';
      for (const part of parts) add(part);
      text = rest;
    }
    open = true;
    active = 0;
  }

  function onblur() {
    open = false;
    if (text.trim()) add(text);
  }
</script>

<div class="wrap">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="inp tokens" onclick={() => input?.focus()}>
    {#each values as value, i (value)}
      <span class="tok">
        {prefix}{value}
        <button
          type="button"
          aria-label="Убрать {prefix}{value}"
          tabindex="-1"
          onclick={() => remove(i)}
        >
          <Icon name="x" size={12} />
        </button>
      </span>
    {/each}
    <input
      bind:this={input}
      bind:value={text}
      {id}
      {placeholder}
      aria-label={label}
      autocomplete="off"
      spellcheck="false"
      role={suggestions ? 'combobox' : undefined}
      aria-expanded={suggestions ? showList : undefined}
      aria-controls={suggestions ? listId : undefined}
      aria-activedescendant={showList ? `${listId}-${active}` : undefined}
      {onkeydown}
      {oninput}
      {onblur}
      onfocus={() => (open = true)}
    />
  </div>

  {#if showList}
    <div class="popup list" class:above id={listId} role="listbox" aria-label="Подходящие варианты">
      {#each options as option, i (option.kind + option.value)}
        <button
          type="button"
          id="{listId}-{i}"
          role="option"
          class="mi"
          class:active={i === active}
          class:create={option.kind === 'new'}
          aria-selected={i === active}
          tabindex="-1"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => add(option.value)}
          onmouseenter={() => (active = i)}
        >
          {#if option.kind === 'existing'}
            {prefix}{option.value}
            <span class="shortcut">
              {option.count}
              {plural(option.count, 'запись', 'записи', 'записей')}
            </span>
          {:else}
            <Icon name="plus" size={14} />Создать {noun} «{option.value}»
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .tokens {
    flex-wrap: wrap;
    gap: 4px;
    padding: 5px 4px 5px 6px;
  }

  .tokens input {
    min-width: 80px;
    height: 24px;
    min-height: 24px;
    padding-left: 4px;
  }

  .tok {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    flex: none;
    max-width: 100%;
    height: 24px;
    padding: 0 2px 0 8px;
    border-radius: 12px;
    background: var(--hover);
    font-size: var(--text-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }

  .tok button {
    display: grid;
    place-items: center;
    flex: none;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }

  .tok button:hover {
    background: var(--pressed);
    color: var(--text);
  }

  .list {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    left: 0;
    z-index: 5;
    padding: 4px;
  }

  .list.above {
    top: auto;
    bottom: calc(100% + 4px);
  }

  .create {
    color: var(--accent);
  }
</style>
