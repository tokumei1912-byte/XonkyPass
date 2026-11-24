<script lang="ts">
  import { tick } from 'svelte';
  import { plural } from '../../lib/format';
  import { normalizeTag } from '../../lib/items';
  import { session } from '../../lib/session.svelte';
  import { deleteTag, openTagMenu, renameTag } from '../../lib/tags';
  import Icon from '../../lib/ui/Icon.svelte';

  let filter = $state('');
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let listbox: HTMLUListElement | undefined = $state();
  const id = $props.id();

  const shown = $derived.by(() => {
    const part = normalizeTag(filter).toLowerCase();
    return part ? session.tags.filter((t) => t.value.toLowerCase().includes(part)) : session.tags;
  });
  const current = $derived(shown[active]);

  $effect(() => {
    input?.focus();
  });

  // После переименования или удаления выделение остаётся в пределах списка.
  $effect(() => {
    if (active >= shown.length) active = Math.max(0, shown.length - 1);
  });

  async function move(to: number) {
    active = Math.max(0, Math.min(to, shown.length - 1));
    await tick();
    listbox?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  /** Список записей с этим тегом. */
  function show(tag: string) {
    session.filter = { kind: 'tag', tag };
    session.query = '';
    session.home();
  }

  function onkeydown(e: KeyboardEvent) {
    const tag = current?.value;
    switch (true) {
      case e.key === 'ArrowDown':
        void move(active + 1);
        break;
      case e.key === 'ArrowUp':
        void move(active - 1);
        break;
      case e.key === 'PageDown':
        void move(active + 8);
        break;
      case e.key === 'PageUp':
        void move(active - 8);
        break;
      case e.key === 'Enter' && !!tag:
        show(tag!);
        break;
      case e.key === 'F2' && !!tag:
        void renameTag(tag!);
        break;
      // В пустом поле Delete нечего стирать — удаляет тег.
      case e.key === 'Delete' && !!tag && !filter:
        void deleteTag(tag!);
        break;
      case (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && !!tag: {
        const row = listbox?.querySelector(`[data-index="${active}"]`);
        if (row) openTagMenu(tag!, row, () => show(tag!));
        break;
      }
      case e.key === 'Escape' && !!filter:
        filter = '';
        break;
      default:
        return;
    }
    e.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="tags" {onkeydown}>
  <header data-tauri-drag-region>
    <button
      type="button"
      class="ib"
      aria-label="Назад (Esc)"
      title="Назад (Esc)"
      onclick={() => session.back()}
    >
      <Icon name="back" />
    </button>
    <h1 data-tauri-drag-region>
      Теги{#if session.tags.length > 0}<span class="count">· {session.tags.length}</span>{/if}
    </h1>
  </header>

  {#if session.tags.length === 0}
    <div class="empty">
      <Icon name="hash" size={40} />
      <h2>Тегов пока нет</h2>
      <p class="muted">
        Добавьте теги в записи — в поле «Теги» или словом <span class="mono">#тег</span> в тексте заметки.
      </p>
    </div>
  {:else}
    <div class="filter">
      <label class="inp">
        <Icon name="search" class="search-icon" />
        <input
          bind:this={input}
          data-return-focus
          bind:value={filter}
          oninput={() => (active = 0)}
          type="search"
          placeholder="Найти тег"
          aria-label="Найти тег"
          role="combobox"
          aria-expanded="true"
          aria-controls="{id}-list"
          aria-activedescendant={current ? `${id}-${active}` : undefined}
          autocomplete="off"
          spellcheck="false"
        />
      </label>
    </div>

    <ul class="rows" bind:this={listbox} id="{id}-list" role="listbox" aria-label="Теги">
      {#each shown as tag, i (tag.value)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          id="{id}-{i}"
          class="row"
          class:active={i === active}
          role="option"
          aria-selected={i === active}
          data-index={i}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => show(tag.value)}
          oncontextmenu={(e) => {
            e.preventDefault();
            active = i;
            openTagMenu(tag.value, e, () => show(tag.value));
          }}
        >
          <span class="hash"><Icon name="hash" size={16} /></span>
          <span class="text">
            <span class="name ellipsis">{tag.value}</span>
            <span class="muted small">
              {tag.count}
              {plural(tag.count, 'запись', 'записи', 'записей')}
            </span>
          </span>
          <span class="actions">
            <button
              type="button"
              class="ib"
              tabindex="-1"
              aria-label="Переименовать #{tag.value}"
              title="Переименовать (F2)"
              onclick={(e) => {
                e.stopPropagation();
                void renameTag(tag.value);
              }}
            >
              <Icon name="pencil" />
            </button>
            <button
              type="button"
              class="ib"
              tabindex="-1"
              aria-label="Удалить #{tag.value}"
              title="Удалить (Del)"
              onclick={(e) => {
                e.stopPropagation();
                void deleteTag(tag.value);
              }}
            >
              <Icon name="trash" />
            </button>
          </span>
        </li>
      {:else}
        <li class="none muted">Нет тегов с «{filter}»</li>
      {/each}
    </ul>

    <p class="keys muted small">
      <kbd>Enter</kbd> записи · <kbd>F2</kbd> переименовать · <kbd>Del</kbd> удалить
    </p>
  {/if}
</main>

<style>
  .tags {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 4px;
  }

  h1 {
    flex: 1;
    font-family: var(--font-ui);
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .count {
    margin-left: 0.3em;
    color: var(--text-2);
    font-weight: 400;
  }

  .filter {
    padding: 4px 12px 10px;
  }

  .filter :global(.search-icon) {
    color: var(--text-2);
  }

  .rows {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 0 0 8px;
    overflow-y: auto;
    list-style: none;
  }

  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 48px;
    margin: 0 6px;
    padding: 0 6px 0 10px;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.active {
    background: var(--accent-soft);
  }

  .row.active::before {
    content: '';
    position: absolute;
    top: 14px;
    left: 0;
    width: 3px;
    height: 20px;
    border-radius: 2px;
    background: var(--accent);
  }

  .hash {
    display: grid;
    flex: none;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 16px;
    background: var(--surface-2);
    color: var(--text-2);
  }

  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .name {
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 2px;
    opacity: 0;
  }

  .row:hover .actions,
  .row.active .actions {
    opacity: 1;
  }

  .none {
    padding: 12px 16px;
  }

  .keys {
    padding: 8px 16px 12px;
    border-top: 1px solid var(--border);
  }

  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 32px;
    color: var(--text-3);
    text-align: center;
  }

  .empty h2 {
    margin-top: 4px;
    color: var(--text);
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .empty p {
    max-width: 280px;
  }
</style>
