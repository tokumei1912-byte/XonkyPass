<script lang="ts">
  import { copyNote, itemMenu, openSite, toggleFavorite } from '../../lib/actions';
  import type { ItemSummary, NoteMeta } from '../../lib/api';
  import { formatRelative } from '../../lib/format';
  import { session } from '../../lib/session.svelte';
  import Avatar from '../../lib/ui/Avatar.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import Markdown from '../../lib/ui/Markdown.svelte';
  import { menu } from '../../lib/ui/overlays.svelte';

  interface Props {
    item: ItemSummary;
  }

  let { item }: Props = $props();

  const meta = $derived(item.meta as NoteMeta);
  let moreButton: HTMLButtonElement | undefined = $state();

  function edit() {
    session.go({ name: 'edit', id: item.id, kind: 'note' });
  }

  function showMenu() {
    if (moreButton)
      menu.openAt(
        moreButton,
        `Действия с заметкой ${meta.title}`,
        itemMenu(item, { inCard: true }),
      );
  }

  function openTag(tag: string) {
    session.filter = { kind: 'tag', tag };
    session.query = '';
    session.home();
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyE') {
      e.preventDefault();
      edit();
    } else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      e.preventDefault();
      showMenu();
    }
  }
</script>

<svelte:window {onkeydown} />

<main class="note-screen">
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
    <div class="grow" data-tauri-drag-region></div>
    <button
      type="button"
      class="ib"
      class:on={meta.favorite}
      aria-label={meta.favorite ? 'Убрать из избранного' : 'Добавить в избранное'}
      aria-pressed={meta.favorite}
      title={meta.favorite ? 'Убрать из избранного' : 'В избранное'}
      onclick={() => toggleFavorite(item)}
    >
      <Icon name="star" filled={meta.favorite} />
    </button>
    <button
      type="button"
      class="ib"
      aria-label="Изменить (Ctrl+E)"
      title="Изменить (Ctrl+E)"
      onclick={edit}
    >
      <Icon name="pencil" />
    </button>
    <button
      bind:this={moreButton}
      type="button"
      class="ib"
      aria-label="Ещё"
      aria-haspopup="menu"
      title="Ещё"
      onclick={showMenu}
    >
      <Icon name="more" />
    </button>
  </header>

  <div class="content">
    <div class="head">
      <Avatar title={meta.title} kind="note" size={48} />
      <div class="head-text">
        <h1 class="selectable">{meta.title}</h1>
        <div class="muted small">Заметка · изменена {formatRelative(item.updated_at)}</div>
      </div>
    </div>

    {#if meta.tags.length > 0}
      <div class="tags">
        {#each meta.tags as tag (tag)}
          <button
            type="button"
            class="chip"
            title="Показать записи с тегом"
            onclick={() => openTag(tag)}
          >
            #{tag}
          </button>
        {/each}
      </div>
    {/if}

    {#if meta.body.trim()}
      <article class="card body">
        <Markdown source={meta.body} onlink={openSite} />
      </article>
    {:else}
      <p class="muted">Заметка пустая.</p>
    {/if}
  </div>

  <footer>
    <button type="button" class="btn" disabled={!meta.body.trim()} onclick={() => copyNote(item)}>
      <Icon name="copy" />Копировать текст
    </button>
    <button type="button" class="btn" onclick={edit}><Icon name="pencil" />Изменить</button>
  </footer>
</main>

<style>
  .note-screen {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 8px 8px 4px;
  }

  .grow {
    flex: 1;
    align-self: stretch;
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
    padding: 4px 16px 16px;
    overflow-y: auto;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .head-text {
    min-width: 0;
  }

  h1 {
    font-size: var(--text-xl);
    font-weight: 600;
    line-height: 1.25;
    overflow-wrap: anywhere;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .body {
    flex: none;
    padding: 14px 16px;
  }

  footer {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }

  footer .btn {
    flex: 1;
  }
</style>
