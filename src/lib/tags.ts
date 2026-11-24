// Переименование, слияние и удаление тегов.

import { reportError } from './actions';
import { api } from './api';
import { normalizeTag } from './items';
import { session } from './session.svelte';
import { dialogs, menu, type MenuEntry } from './ui/overlays.svelte';
import { toasts } from './ui/overlays.svelte';

/** Как `limits::TAG` в ядре. */
const MAX_TAG_LENGTH = 64;

function validate(value: string): string | null {
  const tag = normalizeTag(value);
  if (!tag) return 'Введите название тега';
  if ([...tag].length > MAX_TAG_LENGTH) return `Не длиннее ${MAX_TAG_LENGTH} символов`;
  return null;
}

function changed(count: number): string {
  return `Изменено записей: ${count}`;
}

/** Переименовывает тег; если такой тег уже есть — предлагает объединить. */
export async function renameTag(tag: string): Promise<void> {
  const input = await dialogs.prompt({
    title: `Переименовать #${tag}`,
    message: 'Тег изменится во всех записях, в том числе в корзине и в тексте заметок.',
    label: 'Новое название',
    value: tag,
    confirm: 'Переименовать',
    validate,
  });
  if (input === null) return;
  const to = normalizeTag(input);
  if (to === tag) return;

  const key = tag.toLowerCase();
  const target = session.tags.find(
    (t) => t.value.toLowerCase() === to.toLowerCase() && t.value.toLowerCase() !== key,
  );
  if (target) {
    const ok = await dialogs.confirm({
      title: `Объединить с #${target.value}?`,
      message: `Тег #${target.value} уже есть. Записи с #${tag} получат его, а #${tag} исчезнет.`,
      confirm: 'Объединить',
    });
    if (!ok) return;
  }

  try {
    // При слиянии — написание уже существующего тега.
    const name = target?.value ?? to;
    const count = await api.tagRename(tag, name);
    await session.refresh();
    session.retagged(tag, name);
    toasts.success(
      target ? `Теги объединены в #${name}` : `Тег переименован в #${name}`,
      changed(count),
    );
  } catch (error) {
    reportError(error);
  }
}

/** Убирает тег из всех записей. */
export async function deleteTag(tag: string): Promise<void> {
  const count = session.tags.find((t) => t.value.toLowerCase() === tag.toLowerCase())?.count;
  const ok = await dialogs.confirm({
    title: `Удалить тег #${tag}?`,
    message:
      `Тег уберётся из всех записей${count ? ` (${count})` : ''}, в том числе из корзины. ` +
      'Сами записи останутся.',
    confirm: 'Удалить тег',
    danger: true,
  });
  if (!ok) return;
  try {
    const removed = await api.tagDelete(tag);
    await session.refresh();
    session.retagged(tag, null);
    toasts.success(`Тег #${tag} удалён`, changed(removed));
  } catch (error) {
    reportError(error);
  }
}

/** Пункты меню тега — у чипа в списке и на экране тегов. */
export function tagMenu(tag: string, show?: () => void): MenuEntry[] {
  const entries: MenuEntry[] = [];
  if (show) entries.push({ label: 'Показать записи', icon: 'list', shortcut: 'Enter', run: show });
  entries.push(
    { label: 'Переименовать…', icon: 'pencil', shortcut: 'F2', run: () => renameTag(tag) },
    'separator',
    {
      label: 'Удалить тег…',
      icon: 'trash',
      shortcut: 'Del',
      danger: true,
      run: () => deleteTag(tag),
    },
  );
  return entries;
}

/** Меню тега под элементом (клавиатура) или в точке щелчка. */
export function openTagMenu(tag: string, target: Element | MouseEvent, show?: () => void): void {
  const label = `Действия с тегом ${tag}`;
  if (target instanceof MouseEvent) {
    menu.open({ x: target.clientX, y: target.clientY, label, entries: tagMenu(tag, show) });
  } else {
    menu.openAt(target, label, tagMenu(tag, show));
  }
}
