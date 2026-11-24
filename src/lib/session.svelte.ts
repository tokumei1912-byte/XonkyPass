// Состояние панели: открыто ли хранилище, записи и стек экранов.

import {
  api,
  type ItemId,
  type ItemKind,
  type ItemSummary,
  type SortOrder,
  type VaultStatus,
} from './api';
import { countTags } from './items';
import { loadSort, saveSort } from './search';
import { dialogs, menu, toasts } from './ui/overlays.svelte';

/** Этап жизни хранилища — какой экран показывать вместо списка. */
export type Phase =
  | 'loading'
  | 'failed'
  /** Хранилища нет: первый запуск. */
  | 'setup'
  | 'locked'
  /** Вход по ключу восстановления. */
  | 'recovery'
  /** Открыто ключом восстановления: нужно задать новый мастер-пароль. */
  | 'reset'
  | 'open';

export type Screen =
  | { name: 'list' }
  | { name: 'item'; id: ItemId }
  /** `id: null` — новая запись. */
  | { name: 'edit'; id: ItemId | null; kind: ItemKind }
  | { name: 'trash' }
  /** Все теги: переименование, слияние, удаление. */
  | { name: 'tags' }
  | { name: 'settings' }
  | { name: 'change-password' }
  /** Новый ключ восстановления из настроек. */
  | { name: 'recovery-key'; key: string };

export type ListFilter = { kind: 'all' } | { kind: 'favorites' } | { kind: 'tag'; tag: string };

const HOME: Screen = { name: 'list' };

class Session {
  phase = $state<Phase>('loading');
  /** Текст ошибки для экрана `failed`. */
  error = $state('');
  vaultPath = $state('');
  items = $state.raw<ItemSummary[]>([]);
  /** Записи, которые не удалось расшифровать. */
  damaged = $state.raw<ItemId[]>([]);
  trashCount = $state(0);
  stack = $state.raw<Screen[]>([HOME]);

  /** Состояние списка — сохраняется, пока открыта карточка или редактор. */
  query = $state('');
  /** `all`, `favorites` или тег. */
  filter = $state<ListFilter>({ kind: 'all' });
  selected = $state<ItemId | null>(null);
  /** Порядок списка; запоминается между запусками. */
  sort = $state<SortOrder>(loadSort());

  screen = $derived(this.stack[this.stack.length - 1] ?? HOME);
  tags = $derived(countTags(this.items));

  /**
   * Экран с несохранёнными правками спрашивает, можно ли уйти (Esc, «Отмена»).
   * Ставит его сам экран и снимает при уходе.
   */
  guard: (() => Promise<boolean>) | null = null;

  /** Узнаёт, что с хранилищем, и выбирает первый экран. */
  async start(): Promise<void> {
    try {
      this.applyStatus(await api.vaultStatus());
      if (this.phase === 'open') await this.refresh();
    } catch (error) {
      this.error = String(error instanceof Error ? error.message : error);
      this.phase = 'failed';
    }
  }

  applyStatus(status: VaultStatus): void {
    this.vaultPath = status.path;
    if (!status.exists) this.phase = 'setup';
    else if (!status.unlocked) this.phase = 'locked';
    else if (status.unlocked_with === 'recovery_key') this.phase = 'reset';
    else this.phase = 'open';
  }

  /** Хранилище открыто: показать список. */
  async opened(): Promise<void> {
    this.stack = [HOME];
    this.phase = 'open';
    await this.refresh();
  }

  /** Хранилище заблокировано (кнопкой, из трея, позже — по таймеру): всё забыть. */
  locked(): void {
    menu.close();
    dialogs.close(false);
    toasts.clear();
    this.items = [];
    this.damaged = [];
    this.trashCount = 0;
    this.stack = [HOME];
    this.query = '';
    this.filter = { kind: 'all' };
    this.selected = null;
    if (this.phase === 'setup') {
      // Во время первого запуска хранилища может ещё не быть — спросим Rust.
      void api.vaultStatus().then(
        (status) => this.applyStatus(status),
        () => undefined,
      );
    } else if (this.phase !== 'loading' && this.phase !== 'failed') {
      this.phase = 'locked';
    }
  }

  async refresh(): Promise<void> {
    const [active, trash] = await Promise.all([api.itemsList('active'), api.itemsList('trash')]);
    this.items = active.items;
    this.damaged = active.damaged;
    this.trashCount = trash.items.length;
  }

  setSort(sort: SortOrder): void {
    this.sort = sort;
    saveSort(sort);
  }

  /** Тег переименовали (`to`) или удалили (`null`): фильтр по нему следует за ним. */
  retagged(from: string, to: string | null): void {
    const filter = this.filter;
    if (filter.kind !== 'tag' || filter.tag.toLowerCase() !== from.toLowerCase()) return;
    this.filter = to === null ? { kind: 'all' } : { kind: 'tag', tag: to };
  }

  item(id: ItemId): ItemSummary | undefined {
    return this.items.find((item) => item.id === id);
  }

  /** Обновлённая запись из ответа команды — без перечитывания всего списка. */
  upsert(summary: ItemSummary): void {
    const index = this.items.findIndex((item) => item.id === summary.id);
    this.items =
      index < 0
        ? [...this.items, summary]
        : this.items.map((item, i) => (i === index ? summary : item));
  }

  go(screen: Screen): void {
    menu.close();
    this.stack = [...this.stack, screen];
  }

  replace(screen: Screen): void {
    this.stack = [...this.stack.slice(0, -1), screen];
  }

  back(): void {
    menu.close();
    this.stack = this.stack.length > 1 ? this.stack.slice(0, -1) : [HOME];
  }

  /** Назад, если экран не против (см. `guard`). */
  async leave(): Promise<void> {
    if (this.guard && !(await this.guard())) return;
    this.back();
  }

  home(): void {
    menu.close();
    this.stack = [HOME];
  }

  /** Убирает из стека экраны удалённой записи. */
  forget(id: ItemId): void {
    const kept = this.stack.filter((s) => !('id' in s) || s.id !== id);
    this.stack = kept.length > 0 ? kept : [HOME];
  }
}

export const session = new Session();
