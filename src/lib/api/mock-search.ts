// Поиск для mock-бэкенда: упрощённое повторение `crates/core/src/search.rs`, чтобы
// интерфейс в браузере вёл себя как в приложении. Нечёткое сравнение проще, чем
// в nucleo, но правила те же: все слова, `#тег`, `is:`/`has:`, группы пустого запроса,
// повтор в другой раскладке.

import type { ItemKind, ItemSummary, SearchOptions, SearchResult } from './types';

interface Query {
  words: string[];
  tags: string[];
  kind: ItemKind | null;
  favorite: boolean;
  hasTotp: boolean;
}

const IS_VALUES = ['login', 'note', 'fav', 'favorite'];
const HAS_VALUES = ['totp', '2fa'];

function normalizeTag(tag: string): string {
  return tag.trim().replace(/^#+/, '').trim().split(/\s+/).filter(Boolean).join('-');
}

function parse(input: string): Query {
  const query: Query = { words: [], tags: [], kind: null, favorite: false, hasTotp: false };
  for (const token of input.split(/\s+/).filter(Boolean)) {
    if (token.startsWith('#')) {
      const tag = normalizeTag(token);
      if (tag) query.tags.push(tag);
      continue;
    }
    const colon = token.indexOf(':');
    if (colon > 0) {
      const op = token.slice(0, colon).toLowerCase();
      const value = token.slice(colon + 1).toLowerCase();
      const known = op === 'is' ? IS_VALUES : op === 'has' ? HAS_VALUES : [];
      if (op === 'is' && value === 'login') query.kind = 'login';
      else if (op === 'is' && value === 'note') query.kind = 'note';
      else if (op === 'is' && (value === 'fav' || value === 'favorite')) query.favorite = true;
      else if (op === 'has' && (value === 'totp' || value === '2fa')) query.hasTotp = true;
      else if (!known.some((v) => v.startsWith(value))) query.words.push(token);
      continue;
    }
    query.words.push(token);
  }
  return query;
}

function fold(text: string): string {
  return text.toLowerCase().replaceAll('ё', 'е');
}

/** Буквы слова по порядку; очки за начало слов и подряд идущие буквы. */
function fuzzy(needle: string, haystack: string): number | null {
  let score = 0;
  let from = 0;
  let previous = -2;
  for (const char of needle) {
    const at = haystack.indexOf(char, from);
    if (at < 0) return null;
    score += 16;
    if (at === previous + 1) score += 8;
    if (at === 0 || /[\s.@_\-/:]/.test(haystack[at - 1]!)) score += 10;
    if (at === 0 && previous === -2) score += 10;
    score -= Math.min(at - from, 6);
    previous = at;
    from = at + 1;
  }
  return Math.max(score, 1);
}

const DAY_MS = 24 * 60 * 60 * 1000;

function wordScore(word: string, item: ItemSummary): number | null {
  const { meta } = item;
  const fields: [string, number][] = [[meta.title, 4]];
  if (meta.kind === 'login') {
    fields.push(
      [meta.username, 2],
      ...[...meta.urls, ...meta.apps].map((u): [string, number] => [u, 3]),
    );
  }
  fields.push(...meta.tags.map((t): [string, number] => [t, 3]));
  let best: number | null = null;
  for (const [text, weight] of fields) {
    const score = fuzzy(word, fold(text));
    if (score !== null) best = Math.max(best ?? 0, (score * weight) / 4);
  }
  if (best === null && meta.kind === 'note' && fold(meta.body).includes(word)) {
    best = word.length * 6;
  }
  return best;
}

function bonus(item: ItemSummary, now: number): number {
  let bonus = item.meta.favorite ? 24 : 0;
  if (item.use_count > 0) bonus += Math.min(8 * Math.floor(Math.log2(item.use_count + 1)), 40);
  if (item.last_used_at !== null) {
    const age = now - item.last_used_at;
    bonus += age < DAY_MS ? 24 : age < 7 * DAY_MS ? 16 : age < 30 * DAY_MS ? 8 : 0;
  }
  return bonus;
}

const collator = new Intl.Collator('ru', { sensitivity: 'base', numeric: true });

function run(items: readonly ItemSummary[], query: Query, options: SearchOptions, now: number) {
  const known = new Set(items.flatMap((i) => i.meta.tags.map((t) => t.toLowerCase())));
  const tags = query.tags.map((t) => {
    const key = t.toLowerCase();
    return known.has(key) ? (tag: string) => tag === key : (tag: string) => tag.startsWith(key);
  });
  const words = query.words.map(fold);
  const hits: { score: number; item: ItemSummary }[] = [];
  for (const item of items) {
    const { meta } = item;
    const itemTags = meta.tags.map((t) => t.toLowerCase());
    if (
      (query.kind && meta.kind !== query.kind) ||
      (query.favorite && !meta.favorite) ||
      (query.hasTotp && !(meta.kind === 'login' && meta.has_totp)) ||
      !tags.every((test) => itemTags.some(test))
    ) {
      continue;
    }
    let score = 0;
    let ok = true;
    for (const word of words) {
      const s = wordScore(word, item);
      if (s === null) {
        ok = false;
        break;
      }
      score += s;
    }
    if (!ok) continue;
    if (words.length > 0) score += bonus(item, now);
    hits.push({ score, item });
  }

  let favorites = 0;
  let recent = 0;
  const byWords = words.length > 0;
  if (!byWords && options.sort === 'relevance') {
    const recents = hits
      .filter((h) => !h.item.meta.favorite && h.item.last_used_at !== null)
      .filter((h) => now - h.item.last_used_at! < 7 * DAY_MS)
      .sort((a, b) => b.item.last_used_at! - a.item.last_used_at!)
      .slice(0, 5);
    for (const hit of hits) {
      hit.score = hit.item.meta.favorite ? 2 : recents.includes(hit) ? 1 : 0;
      if (hit.score === 2) favorites += 1;
    }
    recent = recents.length;
  }

  const title = (a: ItemSummary, b: ItemSummary) => collator.compare(a.meta.title, b.meta.title);
  const used = (a: ItemSummary, b: ItemSummary) => (b.last_used_at ?? -1) - (a.last_used_at ?? -1);
  hits.sort(({ score: sa, item: a }, { score: sb, item: b }) => {
    switch (options.sort) {
      case 'relevance':
        return sb - sa || (!byWords && sa > 0 ? used(a, b) : 0) || title(a, b);
      case 'title':
        return title(a, b);
      case 'updated':
        return b.updated_at - a.updated_at || title(a, b);
      case 'used':
        return b.use_count - a.use_count || used(a, b) || title(a, b);
    }
  });
  const limit = options.limit === 0 ? hits.length : options.limit;
  return {
    items: hits.slice(0, limit).map((h) => structuredClone(h.item)),
    total: hits.length,
    layout_fixed: null,
    favorites,
    recent,
  } satisfies SearchResult;
}

const LAYOUT_EN = "qwertyuiop[]asdfghjkl;'zxcvbnm,.`";
const LAYOUT_RU = 'йцукенгшщзхъфывапролджэячсмитьбюё';

function switchLayout(words: string[]): string[] | null {
  const text = words.join('');
  const latin = /[a-z]/i.test(text);
  const cyrillic = /[а-яё]/i.test(text);
  if (latin === cyrillic) return null;
  const [from, to] = latin ? [LAYOUT_EN, LAYOUT_RU] : [LAYOUT_RU, LAYOUT_EN];
  return words.map((word) =>
    [...word.toLowerCase()].map((c) => (from.includes(c) ? to[from.indexOf(c)] : c)).join(''),
  );
}

export function searchItems(
  items: readonly ItemSummary[],
  input: string,
  options: SearchOptions,
  now = Date.now(),
): SearchResult {
  const query = parse(input);
  const result = run(items, query, options, now);
  if (result.total > 0 || query.words.length === 0) return result;
  const words = switchLayout(query.words);
  if (!words) return result;
  const fixed = run(items, { ...query, words }, options, now);
  return fixed.total > 0 ? { ...fixed, layout_fixed: words.join(' ') } : result;
}

/** Меняет тег во всех записях (или убирает его); заметки — и в тексте. */
export function retag(meta: ItemSummary['meta'], from: string, to: string | null): boolean {
  const key = from.toLowerCase();
  const before = JSON.stringify(meta);
  const index = meta.tags.findIndex((t) => t.toLowerCase() === key);
  if (index >= 0) {
    if (to === null) meta.tags.splice(index, 1);
    else meta.tags[index] = to;
  }
  const seen = new Set<string>();
  meta.tags = meta.tags.filter((t) => !seen.has(t.toLowerCase()) && seen.add(t.toLowerCase()));
  if (meta.kind === 'note') {
    meta.body = meta.body.replace(
      /(^|[\s(])#([\p{L}\p{N}][\p{L}\p{N}_-]*)/gu,
      (match: string, before: string, tag: string) =>
        tag.toLowerCase() === key ? `${before}${to === null ? tag : `#${to}`}` : match,
    );
  }
  return JSON.stringify(meta) !== before;
}

export { normalizeTag };
