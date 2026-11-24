<script lang="ts">
  import { onMount } from 'svelte';
  import { reportError } from '../../lib/actions';
  import { api, type ItemSummary } from '../../lib/api';
  import { formatRelative, plural } from '../../lib/format';
  import { session } from '../../lib/session.svelte';
  import Avatar from '../../lib/ui/Avatar.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { dialogs, toasts } from '../../lib/ui/overlays.svelte';

  let items = $state.raw<ItemSummary[] | null>(null);

  onMount(() => {
    void load();
  });

  async function load() {
    try {
      const list = await api.itemsList('trash');
      items = [...list.items].sort((a, b) => (b.deleted_at ?? 0) - (a.deleted_at ?? 0));
    } catch (error) {
      reportError(error);
      items = [];
    }
  }

  /** Убирает запись из списка корзины и обновляет счётчики в списке записей. */
  async function drop(item: ItemSummary) {
    items = (items ?? []).filter((i) => i.id !== item.id);
    await session.refresh();
  }

  const noun = (item: ItemSummary) => (item.meta.kind === 'note' ? 'Заметка' : 'Запись');

  async function restore(item: ItemSummary) {
    try {
      await api.itemRestore(item.id);
      await drop(item);
      toasts.success(`${noun(item)} «${item.meta.title}» восстановлена`);
    } catch (error) {
      reportError(error);
    }
  }

  async function purge(item: ItemSummary) {
    const ok = await dialogs.confirm({
      title: 'Удалить навсегда?',
      message:
        item.meta.kind === 'note'
          ? `Заметка «${item.meta.title}» будет удалена из хранилища. Вернуть её будет нельзя.`
          : `Запись «${item.meta.title}» и история её паролей будут удалены из хранилища. Вернуть их будет нельзя.`,
      confirm: 'Удалить',
      danger: true,
    });
    if (!ok) return;
    try {
      await api.itemPurge(item.id);
      await drop(item);
    } catch (error) {
      reportError(error);
    }
  }

  async function empty() {
    const count = items?.length ?? 0;
    const ok = await dialogs.confirm({
      title: 'Очистить корзину?',
      message: `${count} ${plural(count, 'запись будет удалена', 'записи будут удалены', 'записей будут удалены')} навсегда. Вернуть их будет нельзя.`,
      confirm: 'Очистить',
      danger: true,
    });
    if (!ok) return;
    try {
      const removed = await api.trashEmpty();
      items = [];
      await session.refresh();
      toasts.success(
        `Корзина очищена: ${removed} ${plural(removed, 'запись', 'записи', 'записей')}`,
      );
    } catch (error) {
      reportError(error);
    }
  }
</script>

<main class="trash">
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
      Корзина{#if items && items.length > 0}<span class="count">· {items.length}</span>{/if}
    </h1>
    {#if items && items.length > 0}
      <button type="button" class="btn ghost clear" onclick={empty}>Очистить</button>
    {/if}
  </header>

  {#if items === null}
    <div class="grow"></div>
  {:else if items.length === 0}
    <div class="empty">
      <Icon name="trash" size={40} />
      <h2>Корзина пуста</h2>
      <p class="muted">
        Удалённые записи попадают сюда — их можно вернуть, пока корзина не очищена.
      </p>
    </div>
  {:else}
    <p class="muted small intro">
      Записи из корзины не видны в поиске. Их можно вернуть, пока корзина не очищена.
    </p>
    <ul class="rows" aria-label="Записи в корзине">
      {#each items as item (item.id)}
        <li class="row">
          <Avatar title={item.meta.title} kind={item.meta.kind} />
          <div class="text">
            <div class="title ellipsis">{item.meta.title}</div>
            <div class="muted small ellipsis">
              {item.meta.kind === 'note' ? 'заметка · ' : ''}удалено {formatRelative(
                item.deleted_at ?? item.updated_at,
              )}
            </div>
          </div>
          <button
            type="button"
            class="ib"
            aria-label="Восстановить {item.meta.title}"
            title="Восстановить"
            onclick={() => restore(item)}
          >
            <Icon name="restore" />
          </button>
          <button
            type="button"
            class="ib"
            aria-label="Удалить {item.meta.title} навсегда"
            title="Удалить навсегда"
            onclick={() => purge(item)}
          >
            <Icon name="x" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</main>

<style>
  .trash {
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

  .clear {
    height: 30px;
    color: var(--danger);
  }

  .intro {
    padding: 0 16px 10px;
    white-space: normal;
  }

  .grow {
    flex: 1;
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
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    padding: 0 8px 0 16px;
  }

  .row:hover {
    background: var(--hover);
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-weight: 600;
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
