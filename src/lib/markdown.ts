// Разбор Markdown заметок в дерево, которое рисует `ui/Markdown.svelte`.
//
// Своё небольшое подмножество вместо библиотеки: текст заметки никогда не превращается
// в HTML-строку, поэтому подсунуть в заметку скрипт или чужую разметку нельзя.
// Поддерживается: заголовки, абзацы, списки (вложенные, нумерованные, задачи),
// цитаты, блоки кода, линия; в строке — `код`, **жирный**, *курсив*, ~~зачёркнутый~~,
// [ссылки](адрес) и адреса http(s)://.

export type Inline =
  | { t: 'text'; v: string }
  | { t: 'code'; v: string }
  | { t: 'strong' | 'em' | 'del'; c: Inline[] }
  | { t: 'link'; href: string; c: Inline[] }
  | { t: 'br' };

export interface ListItem {
  /** `null` — обычный пункт, иначе пункт-задача и отмечен ли он. */
  task: boolean | null;
  c: Inline[];
  children: Block[];
}

export type Block =
  | { t: 'heading'; level: number; c: Inline[] }
  | { t: 'paragraph'; c: Inline[] }
  | { t: 'list'; ordered: boolean; start: number; items: ListItem[] }
  | { t: 'code'; v: string }
  | { t: 'quote'; c: Block[] }
  | { t: 'hr' };

const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/;
const FENCE = /^\s*(```|~~~)/;
const HR = /^\s*([-*_])(\s*\1){2,}\s*$/;
const QUOTE = /^\s*>\s?/;
const LIST_ITEM = /^(\s*)([-*+]|\d{1,9}[.)])\s+(.*)$/;
const TASK = /^\[([ xX])\]\s+(.*)$/;

export function parseMarkdown(source: string): Block[] {
  return parseBlocks(source.replace(/\r\n?/g, '\n').split('\n'));
}

function parseBlocks(lines: string[]): Block[] {
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (!line.trim()) {
      i += 1;
      continue;
    }
    const fence = FENCE.exec(line);
    if (fence) {
      const code: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i]!.trim().startsWith(fence[1]!)) {
        code.push(lines[i]!);
        i += 1;
      }
      blocks.push({ t: 'code', v: code.join('\n') });
      i += 1;
      continue;
    }
    const heading = HEADING.exec(line);
    if (heading) {
      blocks.push({ t: 'heading', level: heading[1]!.length, c: parseInline(heading[2]!) });
      i += 1;
      continue;
    }
    if (HR.test(line)) {
      blocks.push({ t: 'hr' });
      i += 1;
      continue;
    }
    if (QUOTE.test(line)) {
      const quoted: string[] = [];
      while (i < lines.length && QUOTE.test(lines[i]!)) {
        quoted.push(lines[i]!.replace(QUOTE, ''));
        i += 1;
      }
      blocks.push({ t: 'quote', c: parseBlocks(quoted) });
      continue;
    }
    const item = LIST_ITEM.exec(line);
    if (item) {
      const [list, next] = parseList(lines, i, item[1]!.length);
      blocks.push(list);
      i = next;
      continue;
    }
    const paragraph: string[] = [];
    while (i < lines.length && lines[i]!.trim() && !startsBlock(lines[i]!)) {
      paragraph.push(lines[i]!.trim());
      i += 1;
    }
    blocks.push({ t: 'paragraph', c: parseInline(paragraph.join('\n')) });
  }
  return blocks;
}

function startsBlock(line: string): boolean {
  return (
    FENCE.test(line) ||
    HEADING.test(line) ||
    HR.test(line) ||
    QUOTE.test(line) ||
    LIST_ITEM.test(line)
  );
}

function indentOf(line: string): number {
  return line.length - line.trimStart().length;
}

/** Список с отступом `indent`, начиная со строки `start`. Возвращает список и следующую строку. */
function parseList(lines: string[], start: number, indent: number): [Block, number] {
  const first = LIST_ITEM.exec(lines[start]!)!;
  const ordered = /\d/.test(first[2]!);
  const items: ListItem[] = [];
  let i = start;

  while (i < lines.length) {
    const line = lines[i]!;
    if (!line.trim()) {
      // Пустая строка не прерывает список, если за ней идёт пункт того же уровня.
      const next = lines.slice(i + 1).findIndex((l) => l.trim());
      const following = next >= 0 ? lines[i + 1 + next]! : '';
      const match = LIST_ITEM.exec(following);
      if (match && indentOf(following) === indent) {
        i += 1 + next;
        continue;
      }
      break;
    }
    const match = LIST_ITEM.exec(line);
    const lineIndent = indentOf(line);
    if (match && lineIndent === indent) {
      if (/\d/.test(match[2]!) !== ordered) break;
      const task = TASK.exec(match[3]!);
      items.push({
        task: task ? task[1] !== ' ' : null,
        c: parseInline(task ? task[2]! : match[3]!),
        children: [],
      });
      i += 1;
      continue;
    }
    if (lineIndent > indent && items.length > 0) {
      const last = items[items.length - 1]!;
      if (match) {
        const [nested, next] = parseList(lines, i, lineIndent);
        last.children.push(nested);
        i = next;
      } else {
        last.c.push({ t: 'br' }, ...parseInline(line.trim()));
        i += 1;
      }
      continue;
    }
    break;
  }
  return [{ t: 'list', ordered, start: ordered ? parseInt(first[2]!, 10) : 1, items }, i];
}

const AUTOLINK = /^https?:\/\/[^\s<>()[\]]+[^\s<>()[\].,;:!?'"»]/;
const LINK = /^\[([^\]]+)\]\(([^)\s]+)\)/;

export function parseInline(text: string): Inline[] {
  const out: Inline[] = [];
  let buffer = '';
  const flush = () => {
    if (buffer) out.push({ t: 'text', v: buffer });
    buffer = '';
  };
  const wrapped = (marker: string, from: number): number => {
    // Закрывающий маркер не сразу после открывающего и не после пробела.
    const end = text.indexOf(marker, from + marker.length + 1);
    return end > 0 && text[end - 1] !== ' ' && text[from + marker.length] !== ' ' ? end : -1;
  };

  let i = 0;
  while (i < text.length) {
    const ch = text[i]!;
    const rest = text.slice(i);

    if (ch === '\\' && i + 1 < text.length && /[\\`*_~[\]()#>-]/.test(text[i + 1]!)) {
      buffer += text[i + 1];
      i += 2;
      continue;
    }
    if (ch === '\n') {
      flush();
      out.push({ t: 'br' });
      i += 1;
      continue;
    }
    if (ch === '`') {
      const end = text.indexOf('`', i + 1);
      if (end > i + 1) {
        flush();
        out.push({ t: 'code', v: text.slice(i + 1, end) });
        i = end + 1;
        continue;
      }
    }
    const double = rest.startsWith('**') ? '**' : rest.startsWith('__') ? '__' : null;
    if (double) {
      const end = wrapped(double, i);
      if (end > 0) {
        flush();
        out.push({ t: 'strong', c: parseInline(text.slice(i + 2, end)) });
        i = end + 2;
        continue;
      }
    }
    if (rest.startsWith('~~')) {
      const end = wrapped('~~', i);
      if (end > 0) {
        flush();
        out.push({ t: 'del', c: parseInline(text.slice(i + 2, end)) });
        i = end + 2;
        continue;
      }
    }
    // `_` внутри слова (snake_case) — не курсив.
    if (ch === '*' || (ch === '_' && !/[\p{L}\p{N}]/u.test(text[i - 1] ?? ''))) {
      const end = wrapped(ch, i);
      if (end > 0) {
        flush();
        out.push({ t: 'em', c: parseInline(text.slice(i + 1, end)) });
        i = end + 1;
        continue;
      }
    }
    if (ch === '[') {
      const link = LINK.exec(rest);
      if (link) {
        flush();
        out.push({ t: 'link', href: link[2]!, c: parseInline(link[1]!) });
        i += link[0].length;
        continue;
      }
    }
    if ((ch === 'h' || ch === 'H') && !/[\p{L}\p{N}]/u.test(text[i - 1] ?? '')) {
      const url = AUTOLINK.exec(rest);
      if (url) {
        flush();
        out.push({ t: 'link', href: url[0], c: [{ t: 'text', v: url[0] }] });
        i += url[0].length;
        continue;
      }
    }
    buffer += ch;
    i += 1;
  }
  flush();
  return out;
}

/** Текст без разметки — для подписи в списке. */
export function plainText(source: string): string {
  return (
    source
      .replace(/```[\s\S]*?```/g, ' ')
      .replace(/^\s*#{1,6}\s+/gm, '')
      .replace(/^\s*([-*+]|\d+[.)])\s+(\[[ xX]\]\s+)?/gm, '')
      .replace(/^\s*>\s?/gm, '')
      .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .replace(/[`*~]/g, '')
      // `_курсив_` — разметка, а `id_ed25519` — нет: убираем только `_` по краям слов.
      .replace(/(^|[^\p{L}\p{N}])_+|_+(?=[^\p{L}\p{N}]|$)/gu, '$1')
      .replace(/\s+/g, ' ')
      .trim()
  );
}

/** Теги, написанные в тексте как `#тег` (не заголовки и не код). */
export function hashtags(source: string): string[] {
  const text = source.replace(/```[\s\S]*?```/g, ' ').replace(/`[^`\n]*`/g, ' ');
  const tags = new Set<string>();
  for (const match of text.matchAll(/(^|[\s(])#([\p{L}\p{N}][\p{L}\p{N}_-]*)/gu)) {
    tags.add(match[2]!);
  }
  return [...tags];
}
