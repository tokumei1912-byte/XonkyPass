// Строка поиска: операторы, подсказки, сортировка.

import type { SortOrder } from './api';
import type { TagCount } from './items';

export interface Operator {
  token: string;
  label: string;
}

/** Операторы запроса (как в `crates/core/src/search.rs`). */
export const OPERATORS: readonly Operator[] = [
  { token: 'is:login', label: 'Пароли' },
  { token: 'is:note', label: 'Заметки' },
  { token: 'is:fav', label: 'Избранное' },
  { token: 'has:totp', label: 'С кодом 2FA' },
];

export const SORT_ORDERS: readonly { value: SortOrder; label: string; hint?: string }[] = [
  { value: 'relevance', label: 'По совпадению', hint: 'без запроса — избранное и недавние' },
  { value: 'title', label: 'По названию' },
  { value: 'updated', label: 'Недавно изменённые' },
  { value: 'used', label: 'Часто используемые' },
];

const SORT_KEY = 'xonkypass.sort';

export function loadSort(): SortOrder {
  try {
    const value = localStorage.getItem(SORT_KEY);
    if (SORT_ORDERS.some((o) => o.value === value)) return value as SortOrder;
  } catch {
    // Нет доступа к хранилищу браузера — порядок по умолчанию.
  }
  return 'relevance';
}

export function saveSort(sort: SortOrder): void {
  try {
    localStorage.setItem(SORT_KEY, sort);
  } catch {
    // Не запомнится — не страшно.
  }
}

/** Слово запроса под курсором. */
export interface Token {
  start: number;
  end: number;
  text: string;
}

export function tokenAt(text: string, caret: number): Token {
  let start = caret;
  while (start > 0 && !/\s/.test(text[start - 1]!)) start -= 1;
  let end = caret;
  while (end < text.length && !/\s/.test(text[end]!)) end += 1;
  return { start, end, text: text.slice(start, end) };
}

export type Suggestion =
  | { kind: 'tag'; value: string; count: number; insert: string }
  | { kind: 'operator'; value: string; label: string; insert: string };

const MAX_SUGGESTIONS = 8;

/** Подсказки для слова под курсором: теги после `#`, операторы `is:` и `has:`. */
export function suggest(token: string, tags: readonly TagCount[], query: string): Suggestion[] {
  const used = new Set(query.toLowerCase().split(/\s+/));
  if (token.startsWith('#')) {
    const part = token.slice(1).toLowerCase();
    const fits = (tag: TagCount) =>
      !used.has(`#${tag.value.toLowerCase()}`) || tag.value.toLowerCase() === part;
    const starts = tags.filter((t) => t.value.toLowerCase().startsWith(part) && fits(t));
    const contains = tags.filter(
      (t) =>
        !t.value.toLowerCase().startsWith(part) && t.value.toLowerCase().includes(part) && fits(t),
    );
    const found = [...starts, ...contains];
    // Тег уже введён полностью — подсказывать нечего.
    if (found.length === 1 && found[0]!.value.toLowerCase() === part) return [];
    return found.slice(0, MAX_SUGGESTIONS).map((t) => ({
      kind: 'tag',
      value: t.value,
      count: t.count,
      insert: `#${t.value}`,
    }));
  }
  const lower = token.toLowerCase();
  if (lower.length < 2) return [];
  return OPERATORS.filter(
    (op) => op.token.startsWith(lower) && op.token !== lower && !used.has(op.token),
  ).map((op) => ({ kind: 'operator', value: op.token, label: op.label, insert: op.token }));
}

/** Подставляет подсказку вместо слова; возвращает новый текст и место курсора. */
export function applySuggestion(
  text: string,
  token: Token,
  insert: string,
): { text: string; caret: number } {
  const before = text.slice(0, token.start);
  const after = text.slice(token.end).replace(/^\s*/, '');
  const next = `${before}${insert} ${after}`;
  return { text: next, caret: before.length + insert.length + 1 };
}
