<script lang="ts">
  import { tick } from 'svelte';
  import {
    copyNote,
    copyPassword,
    copyUsername,
    itemMenu,
    reportError,
    toggleFavorite,
    trashItem,
  } from '../../lib/actions';
  import { api, type ItemSummary, type SearchResult } from '../../lib/api';
  import { dock } from '../../lib/dock.svelte';
  import { subtitle } from '../../lib/items';
  import {
    applySuggestion,
    SORT_ORDERS,
    suggest,
    tokenAt,
    type Suggestion,
  } from '../../lib/search';
  import { session, type ListFilter } from '../../lib/session.svelte';
  import { deleteTag, openTagMenu, renameTag } from '../../lib/tags';
  import Avatar from '../../lib/ui/Avatar.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { menu } from '../../lib/ui/overlays.svelte';

  /** Сколько записей показывать за раз: тысячи строк сразу не нужны. */
  const PAGE = 200;

  let search: HTMLInputElement | undefined = $state();
  let listbox: HTMLDivElement | undefined = $state();
  let chips: HTMLElement | undefined = $state();
  const id = $props.id();

  let result = $state.raw<SearchResult | null>(null);
  let seq = 0;
  /** Запрос показанных результатов: новый запрос — выделение на первой записи. */
  let shownKey: string | null = null;

  /** Чип фильтра — часть запроса для Rust. */
  const filterQuery = $derived.by(() => {
    const filter = session.filter;
    if (filter.kind === 'favorites') return 'is:fav';
    if (filter.kind === 'tag') return `#${filter.tag}`;
    return '';
  });
  const fullQuery = $derived(`${filterQuery} ${session.query}`.trim());
  const pageKey = $derived(`${session.sort}\n${fullQuery}`);

  /** «Показать ещё» добавляет страницу к текущему запросу; новый запрос — снова одна. */
  let pages = $state({ key: '', count: 1 });
  const limit = $derived((pages.key === pageKey ? pages.count : 1) * PAGE);

  // Ищем при любом изменении запроса, порядка, числа страниц или самих записей.
  $effect(() => {
    const query = fullQuery;
    const key = pageKey;
    const options = { sort: session.sort, limit };
    void session.items;
    const mine = ++seq;
    api.itemsSearch(query, options).then((found) => {
      if (mine !== seq) return;
      result = found;
      if (shownKey !== null && shownKey !== key) session.selected = found.items[0]?.id ?? null;
      shownKey = key;
      void tick().then(() => scrollTo(session.selected));
    }, reportError);
  });

  const visible = $derived(result?.items ?? []);
  const hasFavorites = $derived(session.items.some((item) => item.meta.favorite));
  const selectedIndex = $derived(visible.findIndex((item) => item.id === session.selected));
  const current = $derived(visible[selectedIndex]);
  const rest = $derived(result ? result.total - visible.length : 0);

  // Выделение всегда на видимой записи.
  $effect(() => {
    if (selectedIndex < 0 && visible.length > 0) session.selected = visible[0]!.id;
  });

  // Фильтр по тегу, которого больше нет (последнюю запись удалили), сбрасывается.
  $effect(() => {
    const filter = session.filter;
    if (
      filter.kind === 'tag' &&
      !session.tags.some((t) => t.value.toLowerCase() === filter.tag.toLowerCase())
    ) {
      session.filter = { kind: 'all' };
    }
  });

  $effect(() => {
    search?.focus();
  });

  // --- Подсказки в поиске ---

  let caret = $state(0);
  let suggesting = $state(false);
  let activeSuggestion = $state(0);
  const token = $derived(tokenAt(session.query, caret));
  const suggestions = $derived<Suggestion[]>(
    suggesting ? suggest(token.text, session.tags, session.query) : [],
  );

  function updateCaret() {
    caret = search?.selectionStart ?? session.query.length;
  }

  function oninput() {
    updateCaret();
    suggesting = true;
    activeSuggestion = 0;
  }

  async function accept(suggestion: Suggestion) {
    const next = applySuggestion(session.query, token, suggestion.insert);
    session.query = next.text;
    suggesting = false;
    await tick();
    search?.focus();
    search?.setSelectionRange(next.caret, next.caret);
    caret = next.caret;
  }

  /** Клавиши подсказок; `true` — клавиша обработана. */
  function suggestionKey(e: KeyboardEvent): boolean {
    if (suggestions.length === 0) return false;
    switch (e.key) {
      case 'ArrowDown':
        activeSuggestion = (activeSuggestion + 1) % suggestions.length;
        return true;
      case 'ArrowUp':
        activeSuggestion = (activeSuggestion - 1 + suggestions.length) % suggestions.length;
        return true;
      case 'Enter':
      case 'Tab':
        if (e.ctrlKey || e.shiftKey || e.altKey) return false;
        void accept(suggestions[activeSuggestion] ?? suggestions[0]!);
        return true;
      case 'Escape':
        suggesting = false;
        return true;
    }
    return false;
  }

  // --- Список ---

  function isFilter(filter: ListFilter): boolean {
    const now = session.filter;
    if (filter.kind === 'tag') {
      return now.kind === 'tag' && now.tag.toLowerCase() === filter.tag.toLowerCase();
    }
    return now.kind === filter.kind;
  }

  function setFilter(filter: ListFilter) {
    session.filter = filter;
  }

  function scrollTo(itemId: string | null) {
    if (!itemId) return;
    listbox?.querySelector(`[data-id="${itemId}"]`)?.scrollIntoView({ block: 'nearest' });
  }

  async function select(index: number) {
    if (index >= visible.length - 1 && rest > 0) more();
    const item = visible[Math.max(0, Math.min(index, visible.length - 1))];
    if (!item) return;
    session.selected = item.id;
    await tick();
    scrollTo(item.id);
  }

  function more() {
    pages = { key: pageKey, count: (pages.key === pageKey ? pages.count : 1) + 1 };
  }

  /** Заголовок раздела перед записью: избранное и недавние при пустом запросе. */
  function sectionAt(index: number): string | null {
    if (!result || session.filter.kind === 'favorites') return null;
    const { favorites, recent } = result;
    if (favorites + recent === 0) return null;
    if (index === 0 && favorites > 0) return 'Избранное';
    if (index === favorites && recent > 0) return 'Недавние';
    if (index === favorites + recent) return 'Все записи';
    return null;
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

  function sortMenu(e: MouseEvent) {
    const button = e.currentTarget as HTMLElement;
    menu.openAt(
      button,
      'Порядок записей',
      SORT_ORDERS.map((order) => ({
        label: order.label,
        checked: order.value === session.sort,
        run: () => session.setSort(order.value),
      })),
    );
  }

  const sortLabel = $derived(SORT_ORDERS.find((o) => o.value === session.sort)?.label ?? '');

  function onkeydown(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    const inSearch = e.target === search;
    if (inSearch && suggestionKey(e)) {
      e.preventDefault();
      e.stopPropagation();
      return;
    }
    // Стрелки и Enter управляют списком, только когда фокус в поиске: на кнопках
    // Enter нажимает саму кнопку. Esc и Ctrl+F работают отовсюду.
    const anywhere = e.key === 'Escape' || (ctrl && e.code === 'KeyF');
    if (!inSearch && e.target !== e.currentTarget && !anywhere) {
      if (e.key.length === 1 && !ctrl && !e.altKey && !(e.target instanceof HTMLInputElement)) {
        search?.focus();
      }
      return;
    }
    switch (true) {
      case e.key === 'ArrowDown':
        void select(selectedIndex + 1);
        break;
      case e.key === 'ArrowUp':
        void select(Math.max(selectedIndex - 1, 0));
        break;
      case e.key === 'PageDown':
        void select(selectedIndex + 8);
        break;
      case e.key === 'PageUp':
        void select(Math.max(selectedIndex - 8, 0));
        break;
      case ctrl && e.key === 'Home':
        void select(0);
        break;
      case ctrl && e.key === 'End':
        void select(visible.length - 1);
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
      case ctrl && e.code === 'KeyD' && !!current:
        void toggleFavorite(current!);
        break;
      case ctrl && e.code === 'KeyF':
        search?.focus();
        search?.select();
        break;
      case e.key === 'Delete' && !!current && !session.query:
        void trashItem(current!);
        break;
      case (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && !!current:
        showMenu(current!);
        break;
      // Esc: сначала очищает поиск, потом снимает фильтр, потом прячет окно (в Panel).
      case e.key === 'Escape' && !!session.query:
        session.query = '';
        break;
      case e.key === 'Escape' && session.filter.kind !== 'all':
        session.filter = { kind: 'all' };
        break;
      default:
        // Набор текста откуда угодно на экране попадает в поиск.
        if (e.key.length === 1 && !ctrl && !e.altKey && !inSearch) search?.focus();
        return;
    }
    e.preventDefault();
  }

  // --- Чипы: одна точка Tab, внутри — стрелки ---

  function chipButtons(): HTMLButtonElement[] {
    return [...(chips?.querySelectorAll<HTMLButtonElement>('button.chip') ?? [])];
  }

  function onchipkey(e: KeyboardEvent) {
    const buttons = chipButtons();
    const index = buttons.indexOf(e.target as HTMLButtonElement);
    if (index < 0) return;
    const focus = (i: number) => {
      const button = buttons[(i + buttons.length) % buttons.length];
      button?.focus();
      button?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    };
    const tag = (e.target as HTMLElement).dataset['tag'];
    switch (true) {
      case e.key === 'ArrowRight':
        focus(index + 1);
        break;
      case e.key === 'ArrowLeft':
        focus(index - 1);
        break;
      case e.key === 'Home':
        focus(0);
        break;
      case e.key === 'End':
        focus(buttons.length - 1);
        break;
      case e.key === 'ArrowDown':
        search?.focus();
        break;
      case !!tag && e.key === 'F2':
        void renameTag(tag!);
        break;
      case !!tag && e.key === 'Delete':
        void deleteTag(tag!);
        break;
      case !!tag && (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')):
        openTagMenu(tag!, e.target as Element, () => setFilter({ kind: 'tag', tag: tag! }));
        break;
      default:
        // Enter и пробел нажимают кнопку сами; список их не получает.
        if (e.key === 'Enter' || e.key === ' ') e.stopPropagation();
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }

  /** Чип с Tab: выбранный фильтр (как у группы переключателей). */
  function chipTabIndex(filter: ListFilter): number {
    return isFilter(filter) ? 0 : -1;
  }

  function lock() {
    api.vaultLock().catch(reportError);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="list-screen" {onkeydown}>
  <header data-tauri-drag-region>
    <div class="search-box">
      <label class="inp search">
        <Icon name="search" class="search-icon" />
        <input
          bind:this={search}
          data-return-focus
          bind:value={session.query}
          type="search"
          placeholder="Поиск, #тег, is:fav"
          aria-label="Поиск"
          role="combobox"
          aria-expanded="true"
          aria-autocomplete="list"
          aria-controls={suggestions.length > 0 ? `${id}-suggest` : `${id}-list`}
          aria-activedescendant={suggestions.length > 0
            ? `${id}-s-${activeSuggestion}`
            : current
              ? `${id}-${current.id}`
              : undefined}
          autocomplete="off"
          spellcheck="false"
          {oninput}
          onclick={updateCaret}
          onkeyup={(e) => {
            if (
              e.key === 'ArrowLeft' ||
              e.key === 'ArrowRight' ||
              e.key === 'Home' ||
              e.key === 'End'
            )
              updateCaret();
          }}
          onblur={() => (suggesting = false)}
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
      {#if suggestions.length > 0}
        <div class="suggest popup" id="{id}-suggest" role="listbox" aria-label="Подсказки">
          {#each suggestions as suggestion, i (suggestion.value)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              id="{id}-s-{i}"
              class="mi"
              class:active={i === activeSuggestion}
              role="option"
              tabindex="-1"
              aria-selected={i === activeSuggestion}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => accept(suggestion)}
              onmouseenter={() => (activeSuggestion = i)}
            >
              {#if suggestion.kind === 'tag'}
                <Icon name="hash" size={14} />
                <span class="ellipsis">{suggestion.value}</span>
                <span class="shortcut">{suggestion.count}</span>
              {:else}
                <Icon name="sliders" size={14} />
                <span class="mono">{suggestion.value}</span>
                <span class="shortcut">{suggestion.label}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </div>
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
    {#if dock.docked}
      <button
        type="button"
        class="ib"
        class:on={dock.state.pinned}
        aria-pressed={dock.state.pinned}
        aria-label="Закрепить панель (Ctrl+P)"
        title={dock.state.pinned
          ? 'Панель закреплена и не прячется сама (Ctrl+P)'
          : 'Закрепить: панель не будет прятаться сама (Ctrl+P)'}
        onclick={() => dock.togglePin()}
      >
        <Icon name="pin" />
      </button>
    {/if}
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
    <div class="bar">
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <nav class="chips" aria-label="Фильтр" bind:this={chips} onkeydown={onchipkey}>
        <button
          type="button"
          class="chip"
          class:on={isFilter({ kind: 'all' })}
          aria-pressed={isFilter({ kind: 'all' })}
          tabindex={chipTabIndex({ kind: 'all' })}
          onclick={() => setFilter({ kind: 'all' })}
        >
          Все <span class="count">{session.items.length}</span>
        </button>
        {#if hasFavorites || isFilter({ kind: 'favorites' })}
          <button
            type="button"
            class="chip"
            class:on={isFilter({ kind: 'favorites' })}
            aria-pressed={isFilter({ kind: 'favorites' })}
            tabindex={chipTabIndex({ kind: 'favorites' })}
            onclick={() => setFilter({ kind: 'favorites' })}
          >
            <Icon name="star" size={12} filled class="star" />Избранное
          </button>
        {/if}
        {#each session.tags as tag (tag.value)}
          {@const filter = { kind: 'tag', tag: tag.value } as const}
          <button
            type="button"
            class="chip"
            class:on={isFilter(filter)}
            aria-pressed={isFilter(filter)}
            tabindex={chipTabIndex(filter)}
            data-tag={tag.value}
            title="Правая кнопка или Shift+F10 — переименовать, удалить"
            onclick={() => setFilter(filter)}
            oncontextmenu={(e) => {
              e.preventDefault();
              openTagMenu(tag.value, e, () => setFilter(filter));
            }}
          >
            #{tag.value} <span class="count">{tag.count}</span>
          </button>
        {/each}
        {#if session.tags.length > 0}
          <button
            type="button"
            class="chip"
            tabindex="-1"
            title="Все теги"
            onclick={() => session.go({ name: 'tags' })}
          >
            <Icon name="hash" size={12} />Теги…
          </button>
        {/if}
      </nav>
      <button
        type="button"
        class="ib sm sort"
        aria-label="Порядок: {sortLabel}"
        title="Порядок: {sortLabel}"
        aria-haspopup="menu"
        onclick={sortMenu}
      >
        <Icon name="sort" size={16} />
      </button>
    </div>

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

    {#if result && (session.query || session.filter.kind !== 'all')}
      <p class="found muted small" role="status">
        {#if result.total === 0}
          Ничего не найдено
        {:else}
          {#if result.layout_fixed}
            В другой раскладке: «<span class="fixed">{result.layout_fixed}</span>» ·
          {/if}
          Найдено: {result.total}
        {/if}
      </p>
    {/if}

    <div class="rows" bind:this={listbox} id="{id}-list" role="listbox" aria-label="Записи">
      {#each visible as item, i (item.id)}
        {@const selected = item.id === session.selected}
        {@const section = sectionAt(i)}
        {#if section}
          <div class="section" role="presentation">{section}</div>
        {/if}
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
      {#if rest > 0}
        <button type="button" class="btn more" tabindex="-1" onclick={more}>
          Показать ещё {Math.min(rest, PAGE)} из {rest}
        </button>
      {/if}
      {#if result && result.total === 0 && session.query}
        <p class="nothing muted small">
          Слова ищутся в названии, логине, сайтах, тегах и тексте заметок. Можно уточнить:
          <span class="mono">#тег</span>, <span class="mono">is:login</span>,
          <span class="mono">is:note</span>, <span class="mono">is:fav</span>,
          <span class="mono">has:totp</span>.
        </p>
      {/if}
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

  .search-box {
    position: relative;
    flex: 1;
    margin-right: 6px;
  }

  .search :global(.search-icon) {
    color: var(--text-2);
  }

  .suggest {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    left: 0;
    z-index: 20;
    padding: 4px;
    animation: drop var(--duration) var(--ease);
  }

  .suggest :global(svg) {
    flex: none;
    color: var(--text-2);
  }

  .bar {
    display: flex;
    flex: none;
    align-items: center;
    gap: 4px;
    padding: 0 8px 10px 12px;
  }

  .chips {
    display: flex;
    flex: 1;
    gap: 6px;
    min-width: 0;
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
    color: var(--text-2);
    font-weight: 400;
  }

  .chip.on .count {
    color: inherit;
  }

  .sort {
    flex: none;
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

  .fixed {
    color: var(--text);
    font-weight: 600;
  }

  .rows {
    flex: 1;
    min-height: 0;
    padding: 2px 0 8px;
    overflow-y: auto;
  }

  .section {
    padding: 10px 16px 4px;
    color: var(--text-2);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .section:first-child {
    padding-top: 2px;
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

  .more {
    display: flex;
    width: calc(100% - 24px);
    margin: 8px 12px 0;
  }

  .nothing {
    padding: 4px 16px;
    line-height: 1.6;
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

  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
</style>
