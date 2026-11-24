// Состояние всплывающих слоёв: тосты, диалоги подтверждения, контекстное меню.
// Рисуют их `Toasts.svelte`, `DialogHost.svelte` и `ContextMenu.svelte` в корне окна.

import type { IconName } from './icons';

// --- Тосты -------------------------------------------------------------------------

export type ToastTone = 'success' | 'info' | 'error';

export interface ToastAction {
  label: string;
  run: () => unknown;
}

export interface Toast {
  id: number;
  tone: ToastTone;
  icon: IconName;
  title: string;
  detail?: string;
  action?: ToastAction;
  /** Отсчёт до очистки буфера обмена. */
  countdown?: { endsAt: number; total: number };
}

type ToastInput = Omit<Toast, 'id' | 'icon'> & { icon?: IconName };

const TONE_ICONS: Record<ToastTone, IconName> = {
  success: 'check-circle',
  info: 'info',
  error: 'alert-circle',
};

/** Больше тостов в маленьком окне не помещается. */
const MAX_TOASTS = 3;

class ToastStore {
  items = $state<Toast[]>([]);
  #timers = new Map<number, ReturnType<typeof setTimeout>>();
  #next = 1;

  /** Показывает тост; `duration` 0 — пока его не закроют. */
  show(input: ToastInput, duration = 4000): number {
    const id = this.#next++;
    const toast: Toast = { ...input, id, icon: input.icon ?? TONE_ICONS[input.tone] };
    const kept = this.items.slice(-(MAX_TOASTS - 1));
    for (const old of this.items) if (!kept.includes(old)) this.#forget(old.id);
    this.items = [...kept, toast];
    if (duration > 0)
      this.#timers.set(
        id,
        setTimeout(() => this.dismiss(id), duration),
      );
    return id;
  }

  success(title: string, detail?: string): number {
    return this.show({ tone: 'success', title, detail });
  }

  info(title: string, detail?: string): number {
    return this.show({ tone: 'info', title, detail });
  }

  error(title: string, detail?: string): number {
    return this.show({ tone: 'error', title, detail }, 6000);
  }

  /** Действие с отменой: «Запись перемещена в корзину · Отменить». */
  undoable(title: string, undo: () => unknown, icon: IconName = 'trash'): number {
    return this.show({ tone: 'info', icon, title, action: { label: 'Отменить', run: undo } }, 6000);
  }

  /** Скопировано в буфер: отсчёт до очистки и кнопка «Очистить сейчас». */
  copied(title: string, clearAfterSecs: number, clearNow: () => unknown): number {
    this.#dropCountdowns();
    if (clearAfterSecs <= 0) {
      return this.show({ tone: 'success', title, detail: 'Буфер не очистится сам' }, 3000);
    }
    const total = clearAfterSecs * 1000;
    return this.show(
      {
        tone: 'success',
        title,
        countdown: { endsAt: Date.now() + total, total },
        action: { label: 'Очистить сейчас', run: clearNow },
      },
      0,
    );
  }

  /** Буфер очищен (таймером, кнопкой или при блокировке). */
  clipboardCleared(): void {
    if (this.#dropCountdowns())
      this.show({ tone: 'info', icon: 'check', title: 'Буфер обмена очищен' }, 2000);
  }

  dismiss(id: number): void {
    this.#forget(id);
    this.items = this.items.filter((t) => t.id !== id);
  }

  clear(): void {
    for (const toast of this.items) this.#forget(toast.id);
    this.items = [];
  }

  #dropCountdowns(): boolean {
    const counting = this.items.filter((t) => t.countdown);
    for (const toast of counting) this.dismiss(toast.id);
    return counting.length > 0;
  }

  #forget(id: number): void {
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
  }
}

export const toasts = new ToastStore();

// --- Диалоги: подтверждение и ввод строки -----------------------------------------

export interface ConfirmOptions {
  title: string;
  message: string;
  confirm: string;
  cancel?: string;
  /** Необратимое действие: кнопка красная. */
  danger?: boolean;
}

export interface PromptOptions {
  title: string;
  message?: string;
  /** Подпись поля. */
  label: string;
  value?: string;
  confirm: string;
  /** Текст ошибки или `null`, если значение подходит. */
  validate?: (value: string) => string | null;
}

export type OpenDialog =
  | (ConfirmOptions & { kind: 'confirm'; resolve: (ok: boolean) => void })
  | (PromptOptions & { kind: 'prompt'; resolve: (value: string | null) => void });

class DialogStore {
  current = $state<OpenDialog | null>(null);

  /** Спрашивает подтверждение; `true` — пользователь согласился. */
  confirm(options: ConfirmOptions): Promise<boolean> {
    this.close(false);
    return new Promise((resolve) => {
      this.current = { ...options, kind: 'confirm', resolve };
    });
  }

  /** Просит ввести строку; `null` — отменили. */
  prompt(options: PromptOptions): Promise<string | null> {
    this.close(false);
    return new Promise((resolve) => {
      this.current = { ...options, kind: 'prompt', resolve };
    });
  }

  /** Закрывает диалог: `ok` — согласие для подтверждения; ввод при этом отменяется. */
  close(ok: boolean): void {
    const current = this.current;
    this.current = null;
    if (current?.kind === 'prompt') current.resolve(null);
    else current?.resolve(ok);
  }

  /** Ввод готов. */
  submit(value: string): void {
    const current = this.current;
    if (current?.kind !== 'prompt') return;
    this.current = null;
    current.resolve(value);
  }
}

export const dialogs = new DialogStore();

// --- Контекстное меню --------------------------------------------------------------

export type MenuEntry =
  | {
      label: string;
      icon?: IconName;
      shortcut?: string;
      danger?: boolean;
      disabled?: boolean;
      /** Пункт-переключатель: галочка у выбранного (порядок сортировки и т. п.). */
      checked?: boolean;
      run: () => unknown;
    }
  | 'separator';

export interface OpenMenu {
  x: number;
  y: number;
  label: string;
  entries: MenuEntry[];
}

class MenuStore {
  current = $state<OpenMenu | null>(null);

  /** Открывает меню в точке `x`, `y` (координаты окна). */
  open(menu: OpenMenu): void {
    this.current = menu;
  }

  /** Меню под элементом — для клавиатуры (Shift+F10, клавиша меню, кнопка «Ещё»). */
  openAt(element: Element, label: string, entries: MenuEntry[]): void {
    const rect = element.getBoundingClientRect();
    this.open({ x: rect.left + 24, y: rect.bottom - 4, label, entries });
  }

  close(): void {
    this.current = null;
  }
}

export const menu = new MenuStore();
