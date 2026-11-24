<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { openSite, reportError } from '../../lib/actions';
  import { api, emptySecret, type ItemId, type NoteMeta } from '../../lib/api';
  import { mergeTags, normalizeTag, titleFromBody } from '../../lib/items';
  import { hashtags } from '../../lib/markdown';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import type { IconName } from '../../lib/ui/icons';
  import Markdown from '../../lib/ui/Markdown.svelte';
  import { dialogs } from '../../lib/ui/overlays.svelte';
  import Segmented from '../../lib/ui/Segmented.svelte';
  import TokenInput from '../../lib/ui/TokenInput.svelte';

  interface Props {
    /** `null` — новая заметка. */
    id: ItemId | null;
  }

  let { id }: Props = $props();

  // Экран пересоздаётся для каждой заметки ({#key} в Panel) — начальные данные читаются один раз.
  const start = untrack(() => (id ? session.item(id) : undefined));
  const startMeta = start?.meta.kind === 'note' ? start.meta : undefined;

  let title = $state(startMeta?.title ?? '');
  let body = $state(startMeta?.body ?? '');
  let tags = $state<string[]>([...(startMeta?.tags ?? [])]);
  let mode = $state<'text' | 'preview'>('text');
  let saving = $state(false);
  let error = $state('');
  let titleInput: HTMLInputElement | undefined = $state();
  let textarea: HTMLTextAreaElement | undefined = $state();
  const uid = $props.id();

  const MODES = [
    { value: 'text', label: 'Текст' },
    { value: 'preview', label: 'Просмотр' },
  ] as const;

  const snapshot = () => JSON.stringify({ title, body, tags });
  const baseline = snapshot();

  onMount(() => {
    if (id && !start) session.back();
    else if (id) textarea?.focus();
    else titleInput?.focus();
  });

  async function confirmLeave(): Promise<boolean> {
    if (snapshot() === baseline) return true;
    return dialogs.confirm({
      title: 'Не сохранять изменения?',
      message: id ? 'Правки в заметке пропадут.' : 'Новая заметка не будет создана.',
      confirm: 'Не сохранять',
      cancel: 'Продолжить',
      danger: true,
    });
  }

  $effect(() => {
    session.guard = confirmLeave;
    return () => {
      if (session.guard === confirmLeave) session.guard = null;
    };
  });

  async function save() {
    if (saving) return;
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    await tick();

    const text = body.trimEnd();
    const name = title.trim() || titleFromBody(text);
    if (!name) {
      error = 'Добавьте название или текст';
      titleInput?.focus();
      return;
    }
    const meta: NoteMeta = {
      kind: 'note',
      title: name,
      body: text,
      tags: mergeTags(tags, hashtags(body)),
      favorite: startMeta?.favorite ?? false,
    };

    saving = true;
    try {
      if (id) {
        session.upsert(await api.itemUpdate(id, meta, null));
        session.back();
      } else {
        const summary = await api.itemCreate(meta, emptySecret());
        session.upsert(summary);
        session.selected = summary.id;
        session.replace({ name: 'item', id: summary.id });
      }
    } catch (err) {
      reportError(err);
    } finally {
      saving = false;
    }
  }

  // --- Форматирование ---------------------------------------------------------------

  /** Заменяет выделение, сохраняя историю отмены (Ctrl+Z), и выделяет часть вставки. */
  function insert(text: string, selectFrom = text.length, selectTo = selectFrom) {
    const el = textarea;
    if (!el) return;
    el.focus();
    const start = el.selectionStart;
    // execCommand оставляет правку в истории Ctrl+Z; запасной путь — без неё.
    if (!document.execCommand('insertText', false, text)) {
      el.setRangeText(text, start, el.selectionEnd, 'end');
    }
    body = el.value;
    el.setSelectionRange(start + selectFrom, start + selectTo);
  }

  function selection(): { text: string; start: number; end: number } {
    const el = textarea;
    if (!el) return { text: '', start: 0, end: 0 };
    return {
      text: el.value.slice(el.selectionStart, el.selectionEnd),
      start: el.selectionStart,
      end: el.selectionEnd,
    };
  }

  /** Обрамляет выделение: `**жирный**`. Без выделения вставляет образец и выделяет его. */
  function wrap(before: string, after: string, sample: string) {
    const selected = selection().text || sample;
    insert(before + selected + after, before.length, before.length + selected.length);
  }

  /** Ставит префикс в начало каждой выделенной строки (`## `, `- `, `- [ ] `). */
  function prefixLines(prefix: string) {
    const el = textarea;
    if (!el) return;
    const value = el.value;
    const lineStart = value.lastIndexOf('\n', el.selectionStart - 1) + 1;
    const nextBreak = value.indexOf('\n', el.selectionEnd);
    const lineEnd =
      el.selectionEnd > el.selectionStart && value[el.selectionEnd - 1] === '\n'
        ? el.selectionEnd - 1
        : nextBreak < 0
          ? value.length
          : nextBreak;
    const lines = value.slice(lineStart, lineEnd).split('\n');
    const all = lines.every((line) => line.startsWith(prefix));
    const next = lines.map((line) => (all ? line.slice(prefix.length) : prefix + line)).join('\n');
    el.setSelectionRange(lineStart, lineEnd);
    insert(next, next.length);
  }

  function code() {
    const { text } = selection();
    if (text.includes('\n'))
      insert('```\n' + text.replace(/\n$/, '') + '\n```\n', 4, 4 + text.replace(/\n$/, '').length);
    else wrap('`', '`', 'код');
  }

  function link() {
    const text = selection().text || 'текст';
    const url = 'https://';
    const before = `[${text}](`;
    insert(`${before}${url})`, before.length, before.length + url.length);
  }

  const TOOLS: readonly { icon: IconName; label: string; run: () => void }[] = [
    { icon: 'bold', label: 'Жирный (Ctrl+B)', run: () => wrap('**', '**', 'жирный') },
    { icon: 'italic', label: 'Курсив (Ctrl+I)', run: () => wrap('*', '*', 'курсив') },
    { icon: 'heading', label: 'Заголовок', run: () => prefixLines('## ') },
    { icon: 'list', label: 'Список', run: () => prefixLines('- ') },
    { icon: 'list-checks', label: 'Список задач', run: () => prefixLines('- [ ] ') },
    { icon: 'code', label: 'Код', run: code },
    { icon: 'link', label: 'Ссылка (Ctrl+K)', run: link },
  ];

  /** Enter в пункте списка начинает следующий пункт; в пустом — заканчивает список. */
  function continueList(e: KeyboardEvent): boolean {
    const el = textarea;
    if (!el || el.selectionStart !== el.selectionEnd) return false;
    const pos = el.selectionStart;
    const lineStart = el.value.lastIndexOf('\n', pos - 1) + 1;
    const line = el.value.slice(lineStart, pos);
    const match = /^(\s*)([-*+] \[[ xX]\] |[-*+] |(\d+)[.)] )/.exec(line);
    if (!match) return false;
    e.preventDefault();
    if (line.length === match[0].length) {
      el.setSelectionRange(lineStart, pos);
      insert('');
      return true;
    }
    const marker = match[3]
      ? `${Number(match[3]) + 1}${match[2]!.slice(match[3].length)}`
      : match[2]!.replace(/\[[xX]\]/, '[ ]');
    insert(`\n${match[1]}${marker}`);
    return true;
  }

  function onTextKeydown(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && e.code === 'KeyB') {
      e.preventDefault();
      wrap('**', '**', 'жирный');
    } else if (ctrl && e.code === 'KeyI') {
      e.preventDefault();
      wrap('*', '*', 'курсив');
    } else if (ctrl && e.code === 'KeyK') {
      e.preventDefault();
      link();
    } else if (e.key === 'Enter' && !ctrl && !e.shiftKey && !e.altKey) {
      continueList(e);
    }
  }

  function onTitleKeydown(e: KeyboardEvent) {
    if ((e.key === 'Enter' && !(e.ctrlKey || e.metaKey)) || e.key === 'ArrowDown') {
      e.preventDefault();
      textarea?.focus();
      textarea?.setSelectionRange(0, 0);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || dialogs.current) return;
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && (e.code === 'KeyS' || e.key === 'Enter')) {
      e.preventDefault();
      void save();
    }
  }

  async function switchMode(next: 'text' | 'preview') {
    if (next === 'text') {
      await tick();
      textarea?.focus();
    }
  }
</script>

<svelte:window {onkeydown} />

<main class="note-edit">
  <header data-tauri-drag-region>
    <button
      type="button"
      class="ib"
      aria-label="Отмена (Esc)"
      title="Отмена (Esc)"
      onclick={() => session.leave()}
    >
      <Icon name="x" />
    </button>
    <h1 data-tauri-drag-region>Заметка</h1>
    <Segmented options={MODES} bind:value={mode} label="Режим" onchange={switchMode} />
    <button type="button" class="btn accent save" disabled={saving} onclick={save}>
      Сохранить
    </button>
  </header>

  {#if mode === 'text'}
    <div class="toolbar" role="toolbar" aria-label="Форматирование">
      {#each TOOLS as tool (tool.icon)}
        <button
          type="button"
          class="ib"
          aria-label={tool.label}
          title={tool.label}
          onmousedown={(e) => e.preventDefault()}
          onclick={tool.run}
        >
          <Icon name={tool.icon} />
        </button>
      {/each}
    </div>
  {/if}

  <div class="editor" class:preview={mode === 'preview'}>
    {#if mode === 'text'}
      <input
        bind:this={titleInput}
        bind:value={title}
        class="title"
        placeholder={titleFromBody(body) || 'Название'}
        aria-label="Название заметки"
        aria-invalid={error !== ''}
        aria-describedby={error ? `${uid}-error` : undefined}
        autocomplete="off"
        oninput={() => (error = '')}
        onkeydown={onTitleKeydown}
      />
      <textarea
        bind:this={textarea}
        bind:value={body}
        class="body mono"
        aria-label="Текст заметки"
        placeholder="Текст в Markdown: ## заголовок, - список, **жирный**, `код`"
        spellcheck="false"
        oninput={() => (error = '')}
        onkeydown={onTextKeydown}></textarea>
    {:else}
      <div class="preview-title">{title.trim() || titleFromBody(body) || 'Без названия'}</div>
      {#if body.trim()}
        <Markdown source={body} onlink={openSite} />
      {:else}
        <p class="muted">Заметка пустая.</p>
      {/if}
    {/if}
  </div>
  {#if error}
    <div class="field-error error" id="{uid}-error">{error}</div>
  {/if}

  <div class="tags">
    <span class="lbl tags-label">Теги</span>
    <div class="tags-input">
      <TokenInput
        bind:values={tags}
        label="Добавить тег"
        prefix="#"
        placeholder={tags.length ? '' : 'работа, дом…'}
        suggestions={session.tags}
        normalize={normalizeTag}
        above
      />
    </div>
  </div>

  <footer class="muted small">
    Без названия возьмётся первая строка. <span class="tag">#тег</span> в тексте добавляет тег. Ctrl+Enter
    — сохранить.
  </footer>
</main>

<style>
  .note-edit {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px 6px 8px;
  }

  h1 {
    flex: 1;
    min-width: 0;
    font-family: var(--font-ui);
    font-size: var(--text-lg);
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .save {
    height: 30px;
  }

  .toolbar {
    display: flex;
    gap: 2px;
    padding: 2px 12px 8px;
  }

  .editor {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    margin: 0 12px;
    padding: 10px 14px 0;
    border: 1px solid var(--border);
    border-bottom-color: var(--border-strong);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .editor:focus-within {
    border-bottom: 2px solid var(--accent);
  }

  .editor.preview {
    padding-bottom: 12px;
    overflow-y: auto;
  }

  .title {
    flex: none;
    height: 28px;
    margin-bottom: 4px;
    padding: 0;
    border: 0;
    outline: 0;
    background: none;
    font-size: 16px;
    font-weight: 600;
  }

  .body {
    flex: 1;
    min-height: 0;
    padding: 0 0 10px;
    border: 0;
    outline: 0;
    background: none;
    font-size: 13px;
    line-height: 1.55;
    resize: none;
    user-select: text;
  }

  .preview-title {
    margin-bottom: 8px;
    font-size: 16px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .error {
    margin: 6px 16px 0;
  }

  .tags {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px 0;
  }

  .tags-label {
    margin: 9px 0 0;
  }

  .tags-input {
    flex: 1;
    min-width: 0;
  }

  .tag {
    color: var(--accent);
  }

  footer {
    padding: 10px 16px 12px;
  }
</style>
