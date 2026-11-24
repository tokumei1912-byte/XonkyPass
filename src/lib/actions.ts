// Действия с записями, общие для списка, карточки и меню: копирование, корзина,
// избранное. Ошибки показываются тостом.

import { api, errorText, type CopyTarget, type ItemSummary } from './api';
import { session } from './session.svelte';
import { toasts, type MenuEntry } from './ui/overlays.svelte';

/** Показать ошибку команды тостом. */
export function reportError(error: unknown): void {
  toasts.error(errorText(error));
}

function clearClipboard(): void {
  api.clipboardClear().catch(reportError);
}

/** Копирует поле записи (сам Rust: значение не проходит через интерфейс). */
export async function copyField(
  item: ItemSummary,
  target: CopyTarget,
  title: string,
): Promise<void> {
  try {
    const { clear_after_secs } = await api.itemCopy(item.id, target);
    toasts.copied(title, clear_after_secs, clearClipboard);
  } catch (error) {
    reportError(error);
  }
}

export function copyPassword(item: ItemSummary): Promise<void> {
  return copyField(item, { field: 'password' }, `Пароль ${item.meta.title} скопирован`);
}

export function copyUsername(item: ItemSummary): Promise<void> {
  return copyField(item, { field: 'username' }, `Логин ${item.meta.title} скопирован`);
}

export function copyNote(item: ItemSummary): Promise<void> {
  return copyField(item, { field: 'body' }, 'Текст заметки скопирован');
}

/** Копирует текст, который уже есть в интерфейсе (сгенерированный пароль, ключ). */
export async function copyText(text: string, title: string): Promise<void> {
  try {
    const { clear_after_secs } = await api.clipboardCopy(text);
    toasts.copied(title, clear_after_secs, clearClipboard);
  } catch (error) {
    reportError(error);
  }
}

export function openSite(url: string): void {
  api.openUrl(url).catch(reportError);
}

export async function toggleFavorite(item: ItemSummary): Promise<void> {
  try {
    const meta = { ...item.meta, favorite: !item.meta.favorite };
    session.upsert(await api.itemUpdate(item.id, meta, null));
  } catch (error) {
    reportError(error);
  }
}

/** В корзину с возможностью отменить. */
export async function trashItem(item: ItemSummary): Promise<void> {
  try {
    await api.itemTrash(item.id);
    session.forget(item.id);
    await session.refresh();
    toasts.undoable(`«${item.meta.title}» перемещён в корзину`, async () => {
      try {
        await api.itemRestore(item.id);
        await session.refresh();
      } catch (error) {
        reportError(error);
      }
    });
  } catch (error) {
    reportError(error);
  }
}

/** Пункты меню записи — в списке (правая кнопка) и в карточке («Ещё»). */
export function itemMenu(item: ItemSummary, options: { inCard?: boolean } = {}): MenuEntry[] {
  const entries: MenuEntry[] = [];
  if (item.meta.kind === 'login') {
    const login = item.meta;
    entries.push(
      {
        label: 'Копировать пароль',
        icon: 'key',
        shortcut: options.inCard ? undefined : 'Enter',
        run: () => copyPassword(item),
      },
      {
        label: 'Копировать логин',
        icon: 'user',
        shortcut: options.inCard ? undefined : 'Shift+Enter',
        disabled: !login.username,
        run: () => copyUsername(item),
      },
    );
    const site = login.urls[0];
    if (site) entries.push({ label: `Открыть ${site}`, icon: 'globe', run: () => openSite(site) });
  } else {
    entries.push({ label: 'Копировать текст', icon: 'copy', run: () => copyNote(item) });
  }
  entries.push(
    'separator',
    {
      label: 'Изменить',
      icon: 'pencil',
      shortcut: 'Ctrl+E',
      run: () => session.go({ name: 'edit', id: item.id, kind: item.meta.kind }),
    },
    {
      label: item.meta.favorite ? 'Убрать из избранного' : 'В избранное',
      icon: 'star',
      run: () => toggleFavorite(item),
    },
    'separator',
    {
      label: 'В корзину',
      icon: 'trash',
      shortcut: 'Del',
      danger: true,
      run: () => trashItem(item),
    },
  );
  return entries;
}
