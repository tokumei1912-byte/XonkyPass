<script lang="ts">
  import { tick } from 'svelte';
  import {
    copyNote,
    copyPassword,
    copyUsername,
    itemMenu,
    reportError,
    trashItem,
  } from '../../lib/actions';
  import { api, type ItemSummary } from '../../lib/api';
  import { byTitle, matches, passesFilter, subtitle } from '../../lib/items';
  import { session, type ListFilter } from '../../lib/session.svelte';
  import Avatar from '../../lib/ui/Avatar.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { menu } from '../../lib/ui/overlays.svelte';

  let search: HTMLInputElement | undefined = $state();
  let listbox: HTMLDivElement | undefined = $state();
  const id = $props.id();

  const visible = $derived(
    session.items
      .filter((item) => passesFilter(item, session.filter) && matches(item, session.query))
      .sort(byTitle),
  );
  const hasFavorites = $derived(session.items.some((item) => item.meta.favorite));
  const selectedIndex = $derived(visible.findIndex((item) => item.id === session.selected));
  const current = $derived(visible[selectedIndex]);

  // Выделение всегда на видимой записи: после поиска — на первой найденной.
  $effect(() => {
    if (selectedIndex < 0 && visible.length > 0) session.selected = visible[0]!.id;
  });

  // Фильтр по тегу, которого больше нет (последнюю запись удалили), сбрасывается.
  $effect(() => {
    const filter = session.filter;
    if (filter.kind === 'tag' && !session.tags.some((t) => t.value === filter.tag)) {
      session.filter = { kind: 'all' };
    }
  });

  $effect(() => {
    search?.focus();
  });

  function isFilter(filter: ListFilter): boolean {
    const now = session.filter;
    if (filter.kind === 'tag') return now.kind === 'tag' && now.tag === filter.tag;
    return now.kind === filter.kind;
  }

  async function select(index: number) {
    const item = visible[index];
    if (!item) return;
    session.selected = item.id;
    await tick();
    listbox?.querySelector(`[data-id="${item.id}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  function open(item: ItemSummary) {
    session.selected = item.id;
    session.go({ name: 'item', id: item.id });
  }

  function primary(item: ItemSummary) {
    if (item.meta.kind === 'login') void copyPassword(item);
    else open(item);
  }

  function showMenu(item: ItemSummary, e?: MouseEvent) {
    session.selected = item.id;
    const label = `Действия с записью ${item.meta.title}`;
    if (e) menu.open({ x: e.clientX, y: e.clientY, label, entries: itemMenu(item) });
    else {
      const row = listbox?.querySelector(`[data-id="${item.id}"]`);
      if (row) menu.openAt(row, label, itemMenu(item));
    }
  }

  function onkeydown(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    switch (true) {
      case e.key === 'ArrowDown':
        void select(Math.min(selectedIndex + 1, visible.length - 1));
        break;
      case e.key === 'ArrowUp':
        void select(Math.max(selectedIndex - 1, 0));
        break;
      case e.key === 'PageDown':
        void select(Math.min(selectedIndex + 8, visible.length - 1));
        break;
      case e.key === 'PageUp':
        void select(Math.max(selectedIndex - 8, 0));
        break;
      case e.key === 'Enter' && !!current && ctrl:
        open(current!);
        break;
      case e.key === 'Enter' && !!current && e.shiftKey:
        if (current!.meta.kind === 'login') void copyUsername(current!);
        break;
      case e.key === 'Enter' && !!current:
        primary(current!);
        break;
      case ctrl && e.code === 'KeyE' && !!current:
        session.go({ name: 'edit', id: current!.id, kind: current!.meta.kind });
        break;
      case e.key === 'Delete' && !!current && !session.query:
        void trashItem(current!);
        break;
      case (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && !!current:
        showMenu(current!);
        break;
      case e.key === 'Escape' && !!session.query:
        session.query = '';
        break;
      default:
        // Набор текста откуда угодно на экране попадает в поиск.
        if (e.key.length === 1 && !ctrl && !e.altKey && e.target !== search) search?.focus();
        return;
    }
    e.preventDefault();
  }

  function lock() {
    api.vaultLock().catch(reportError);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="list-screen" {onkeydown}>
  <header data-tauri-drag-region>
    <label class="inp search">
      <Icon name="search" class="search-icon" />
      <input
        bind:this={search}
        bind:value={session.query}
        type="search"
        placeholder="Поиск"
        aria-label="Поиск"
        role="combobox"
        aria-expanded="true"
        aria-controls="{id}-list"
        aria-activedescendant={current ? `${id}-${current.id}` : undefined}
        autocomplete="off"
        spellcheck="false"
      />
      {#if session.query}
        <button
          type="button"
          class="ib sm"
          aria-label="Очистить поиск"
          onclick={() => {
            session.query = '';
            search?.focus();
          }}
        >
          <Icon name="x" size={14} />
        </button>
      {/if}
    </label>
    <button
      type="button"
      class="ib"
      aria-label="Настройки"
      title="Настройки"
      onclick={() => session.go({ name: 'settings' })}
    >
      <Icon name="settings" />
    </button>
    <button
      type="button"
      class="ib"
      aria-label="Заблокировать (Ctrl+L)"
      title="Заблокировать (Ctrl+L)"
      onclick={lock}
    >
      <Icon name="lock" />
    </button>
  </header>

  {#if session.items.length === 0}
    <div class="empty">
      <span class="empty-icon"><Icon name="key" size={30} /></span>
      <h1>Хранилище готово</h1>
      <p class="muted">
        Здесь пока пусто. Добавьте первый пароль или заметку — всё шифруется прямо на этом
        компьютере.
      </p>
      <div class="empty-actions">
        <button
          type="button"
          class="btn accent lg"
          onclick={() => session.go({ name: 'edit', id: null, kind: 'login' })}
        >
          <Icon name="plus" />Новый пароль
        </button>
        <button
          type="button"
          class="btn lg"
          onclick={() => session.go({ name: 'edit', id: null, kind: 'note' })}
        >
          <Icon name="plus" />Новая заметка
        </button>
      </div>
      <p class="muted small keys"><kbd>Ctrl+N</kbd> пароль · <kbd>Ctrl+Shift+N</kbd> заметка</p>
    </div>
  {:else}
    <nav class="chips" aria-label="Фильтр по тегам">
      <button
        type="button"
        class="chip"
        class:on={isFilter({ kind: 'all' })}
        onclick={() => (session.filter = { kind: 'all' })}
      >
        Все <span class="count">{session.items.length}</span>
      </button>
      {#if hasFavorites}
        <button
          type="button"
          class="chip"
          class:on={isFilter({ kind: 'favorites' })}
          onclick={() => (session.filter = { kind: 'favorites' })}
        >
          <Icon name="star" size={12} filled class="star" />Избранное
        </button>
      {/if}
      {#each session.tags as tag (tag.value)}
        <button
          type="button"
          class="chip"
          class:on={isFilter({ kind: 'tag', tag: tag.value })}
          onclick={() => (session.filter = { kind: 'tag', tag: tag.value })}
        >
          #{tag.value}
        </button>
      {/each}
    </nav>

    {#if session.damaged.length > 0}
      <div class="note danger damaged" role="alert">
        <Icon name="alert-circle" size={18} />
        <p>
          {session.damaged.length === 1
            ? 'Одна запись повреждена'
            : `Повреждено записей: ${session.damaged.length}`}
          и не может быть прочитана. Файл хранилища мог быть испорчен.
        </p>
      </div>
    {/if}

    {#if session.query}
      <p class="found muted small">
        {visible.length === 0 ? 'Ничего не найдено' : `Найдено: ${visible.length}`}
      </p>
    {/if}

    <div class="rows" bind:this={listbox} id="{id}-list" role="listbox" aria-label="Записи">
      {#each visible as item (item.id)}
        {@const selected = item.id === session.selected}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          class:selected
          role="option"
          id="{id}-{item.id}"
          data-id={item.id}
          aria-selected={selected}
          tabindex="-1"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => open(item)}
          oncontextmenu={(e) => {
            e.preventDefault();
            showMenu(item, e);
          }}
        >
          <Avatar title={item.meta.title} kind={item.meta.kind} />
          <span class="text">
            <span class="title">
              <span class="ellipsis">{item.meta.title}</span>
              {#if item.meta.favorite}
                <Icon name="star" size={12} filled label="в избранном" class="star" />
              {/if}
            </span>
            <span class="sub ellipsis">{subtitle(item)}</span>
          </span>
          <span class="quick">
            {#if item.meta.kind === 'login'}
              {#if item.meta.username}
                <button
                  type="button"
                  class="ib"
                  tabindex="-1"
                  aria-label="Копировать логин"
                  title="Копировать логин (Shift+Enter)"
                  onclick={(e) => {
                    e.stopPropagation();
                    void copyUsername(item);
                  }}
                >
                  <Icon name="user" />
                </button>
              {/if}
              <button
                type="button"
                class="ib"
                tabindex="-1"
                aria-label="Копировать пароль"
                title="Копировать пароль (Enter)"
                onclick={(e) => {
                  e.stopPropagation();
                  void copyPassword(item);
                }}
              >
                <Icon name="key" />
              </button>
            {:else}
              <button
                type="button"
                class="ib"
                tabindex="-1"
                aria-label="Копировать текст"
                title="Копировать текст"
                onclick={(e) => {
                  e.stopPropagation();
                  void copyNote(item);
                }}
              >
                <Icon name="copy" />
              </button>
            {/if}
          </span>
        </div>
      {/each}
    </div>

    <footer>
      <button
        type="button"
        class="btn accent"
        title="Новый пароль (Ctrl+N)"
        onclick={() => session.go({ name: 'edit', id: null, kind: 'login' })}
      >
        <Icon name="plus" />Пароль
      </button>
      <button
        type="button"
        class="btn"
        title="Новая заметка (Ctrl+Shift+N)"
        onclick={() => session.go({ name: 'edit', id: null, kind: 'note' })}
      >
        <Icon name="plus" />Заметка
      </button>
      <button
        type="button"
        class="ib trash"
        aria-label="Корзина{session.trashCount ? `: ${session.trashCount}` : ''}"
        title="Корзина"
        onclick={() => session.go({ name: 'trash' })}
      >
        <Icon name="trash" />
        {#if session.trashCount > 0}<span class="badge">{session.trashCount}</span>{/if}
      </button>
    </footer>
  {/if}
</main>

<style>
  .list-screen {
    display: flex;
    flex-direction: column;
    height: 100%;
    outline: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 12px 8px 10px 12px;
  }

  .search {
    flex: 1;
    margin-right: 6px;
  }

  .search :global(.search-icon) {
    color: var(--text-2);
  }

  .chips {
    display: flex;
    flex: none;
    gap: 6px;
    padding: 0 12px 10px;
    overflow-x: auto;
    scrollbar-width: none;
    mask-image: linear-gradient(90deg, #000 85%, transparent);
  }

  .chips::-webkit-scrollbar {
    display: none;
  }

  .chips :global(.star) {
    color: var(--accent);
  }

  .count {
    font-weight: 400;
  }

  .damaged {
    margin: 0 12px 8px;
  }

  .damaged :global(svg) {
    flex: none;
    color: var(--danger);
  }

  .found {
    padding: 0 16px 6px;
  }

  .rows {
    flex: 1;
    min-height: 0;
    padding: 2px 0 8px;
    overflow-y: auto;
  }

  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 52px;
    margin: 0 6px;
    padding: 0 6px 0 8px;
    border-radius: var(--radius);
    outline: 0;
    cursor: pointer;
  }

  .row:hover {
    background: var(--hover);
  }

  .row.selected {
    background: var(--accent-soft);
  }

  .row.selected::before {
    content: '';
    position: absolute;
    top: 16px;
    left: 0;
    width: 3px;
    height: 20px;
    border-radius: 2px;
    background: var(--accent);
  }

  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-weight: 600;
  }

  .title :global(.star) {
    color: var(--accent);
  }

  .sub {
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  .quick {
    display: flex;
    gap: 2px;
    opacity: 0;
  }

  .row:hover .quick,
  .row.selected .quick {
    opacity: 1;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 8px 10px 12px;
    border-top: 1px solid var(--border);
  }

  footer .btn {
    flex: 1;
  }

  .trash {
    position: relative;
  }

  .badge {
    position: absolute;
    top: 3px;
    right: 2px;
    min-width: 15px;
    height: 15px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--text-2);
    color: var(--bg);
    font-size: 10px;
    font-weight: 600;
    line-height: 15px;
  }

  :global(:root[data-mica]) .badge {
    color: var(--popup);
  }

  .empty {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 0 40px 40px;
    text-align: center;
  }

  .empty-icon {
    display: grid;
    place-items: center;
    width: 64px;
    height: 64px;
    margin-bottom: 4px;
    border-radius: 32px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .empty h1 {
    font-size: 18px;
    font-weight: 600;
  }

  .empty-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
    margin-top: 8px;
  }

  .keys {
    margin-top: 10px;
  }
</style>
