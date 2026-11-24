// Подписи записей, названия по умолчанию, теги.

import type { ItemSummary } from './api';
import { plainText } from './markdown';

/** Вторая строка в списке: «логин · сайт» или начало заметки. */
export function subtitle(item: ItemSummary): string {
  const { meta } = item;
  if (meta.kind === 'note') {
    const text = plainText(meta.body);
    // Первая строка часто и есть название — не повторяем её.
    const rest = text.startsWith(meta.title) ? text.slice(meta.title.length).trim() : text;
    return rest ? `Заметка · ${rest.slice(0, 120)}` : 'Заметка';
  }
  const place = meta.urls[0] ?? meta.apps[0];
  return [meta.username, place].filter(Boolean).join(' · ') || 'Без логина';
}

/** Название по сайту или приложению, если его не ввели: `github.com` → `Github`. */
export function titleFrom(urls: readonly string[], apps: readonly string[]): string {
  const url = urls[0];
  if (url) {
    const host = url.replace(/^[a-z][a-z0-9+.-]*:\/\//i, '').split(/[/?#:@]/)[0] ?? '';
    const labels = host
      .replace(/^www\./i, '')
      .split('.')
      .filter(Boolean);
    const name = labels.length > 1 ? labels[labels.length - 2] : labels[0];
    if (name) return name.charAt(0).toUpperCase() + name.slice(1);
  }
  return apps[0]?.replace(/\.exe$/i, '') ?? '';
}

/** Название заметки по первой непустой строке текста. */
export function titleFromBody(body: string): string {
  const line = body.split('\n').find((l) => l.trim()) ?? '';
  return plainText(line).slice(0, 80).trim();
}

export interface TagCount {
  value: string;
  count: number;
}

/** Теги всех записей с числом записей; самые частые — первыми. */
export function countTags(items: readonly ItemSummary[]): TagCount[] {
  const counts = new Map<string, TagCount>();
  for (const item of items) {
    for (const tag of item.meta.tags) {
      const key = tag.toLowerCase();
      const entry = counts.get(key);
      if (entry) entry.count += 1;
      else counts.set(key, { value: tag, count: 1 });
    }
  }
  return [...counts.values()].sort(
    (a, b) => b.count - a.count || a.value.localeCompare(b.value, 'ru'),
  );
}

/** Объединяет списки тегов: единый вид, без повторов (регистр не важен). */
export function mergeTags(...lists: readonly (readonly string[])[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of lists.flat()) {
    const tag = normalizeTag(raw);
    const key = tag.toLowerCase();
    if (tag && !seen.has(key)) {
      seen.add(key);
      result.push(tag);
    }
  }
  return result;
}

/** Тег в едином виде, как его хранит ядро: без `#`, пробелы → `-`. */
export function normalizeTag(value: string): string {
  return value.trim().replace(/^#+/, '').trim().split(/\s+/).filter(Boolean).join('-');
}
