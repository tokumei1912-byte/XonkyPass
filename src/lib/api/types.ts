// Типы данных IPC. Совпадают с тем, как их сериализует Rust (serde):
// имена полей в snake_case, записи различаются по полю `kind`.

/** UUIDv7 записи. */
export type ItemId = string;

export type Scope = 'active' | 'trash';

export type UnlockMethod = 'password' | 'recovery_key';

export interface LoginMeta {
  kind: 'login';
  title: string;
  username: string;
  /** Сайты: домены или адреса, как их ввёл пользователь. */
  urls: string[];
  /** Приложения: имена exe. */
  apps: string[];
  tags: string[];
  favorite: boolean;
  /** Есть ли код 2FA. Хранилище выставляет само. */
  has_totp: boolean;
}

export interface NoteMeta {
  kind: 'note';
  title: string;
  /** Текст в Markdown. */
  body: string;
  tags: string[];
  favorite: boolean;
}

export type ItemMeta = LoginMeta | NoteMeta;

export type ItemKind = ItemMeta['kind'];

export interface CustomField {
  name: string;
  value: string;
  hidden: boolean;
}

/** Секретная часть записи. Попадает в интерфейс только по явному действию. */
export interface ItemSecret {
  password: string;
  /** Ссылка `otpauth://…` или секрет 2FA; пустая строка — нет 2FA. */
  totp: string;
  notes: string;
  fields: CustomField[];
}

/** Запись без секретов: для списка и карточки. Время — unix, миллисекунды. */
export interface ItemSummary {
  id: ItemId;
  meta: ItemMeta;
  created_at: number;
  updated_at: number;
  /** Не `null` — запись в корзине. */
  deleted_at: number | null;
  revision: number;
  use_count: number;
  last_used_at: number | null;
}

export interface ItemList {
  items: ItemSummary[];
  /** Записи, которые не удалось расшифровать (файл повреждён или подменён). */
  damaged: ItemId[];
}

/** Порядок результатов поиска. */
export type SortOrder =
  /** По совпадению; пустой запрос — сначала избранное и недавние. */
  | 'relevance'
  | 'title'
  /** Недавно изменённые первыми. */
  | 'updated'
  /** Часто используемые первыми. */
  | 'used';

export interface SearchOptions {
  sort: SortOrder;
  /** Сколько записей вернуть; 0 — все. */
  limit: number;
}

export interface SearchResult {
  /** Не больше `limit` записей. */
  items: ItemSummary[];
  /** Сколько всего записей подходит под запрос. */
  total: number;
  /** Слова запроса в другой раскладке, если нашлось только по ним. */
  layout_fixed: string | null;
  /**
   * Запрос без слов по совпадению: первые `favorites` записей — избранные,
   * за ними `recent` недавних, дальше остальные по названию. Иначе 0.
   */
  favorites: number;
  recent: number;
}

export interface VaultStatus {
  /** Есть ли файл хранилища. Нет — первый запуск. */
  exists: boolean;
  unlocked: boolean;
  /** После ключа восстановления нужно задать новый мастер-пароль. */
  unlocked_with: UnlockMethod | null;
  path: string;
}

export interface PasswordOptions {
  length: number;
  lowercase: boolean;
  uppercase: boolean;
  digits: boolean;
  symbols: boolean;
  /** Не использовать похожие символы (`0`/`O`, `1`/`l`/`I`). */
  avoid_similar: boolean;
}

export interface PassphraseOptions {
  words: number;
  separator: string;
  capitalize: boolean;
  /** Добавить цифру к одному из слов. */
  number: boolean;
}

/** Ограничения генератора (как в `xonkypass_core::generator`). */
export const PASSWORD_LENGTH = { min: 4, max: 128 } as const;
export const PASSPHRASE_WORDS = { min: 3, max: 20 } as const;

export const DEFAULT_PASSWORD_OPTIONS: PasswordOptions = {
  length: 20,
  lowercase: true,
  uppercase: true,
  digits: true,
  symbols: true,
  avoid_similar: false,
};

export const DEFAULT_PASSPHRASE_OPTIONS: PassphraseOptions = {
  words: 6,
  separator: '-',
  capitalize: true,
  number: true,
};

export interface Strength {
  /** 0 — очень слабый … 4 — сильный. */
  score: 0 | 1 | 2 | 3 | 4;
  /** Десятичный логарифм числа попыток для подбора. */
  guesses_log10: number;
}

/** Что скопировать из записи. Само копирование делает Rust. */
export type CopyTarget =
  | { field: 'username' }
  | { field: 'password' }
  | { field: 'notes' }
  | { field: 'body' }
  | { field: 'custom'; index: number };

export interface Copied {
  /** Через сколько секунд буфер очистится; 0 — не очистится. */
  clear_after_secs: number;
}

export type Theme = 'system' | 'light' | 'dark';

/** Где живёт панель: у края экрана или обычным окном. */
export type DockMode = 'left' | 'right' | 'top' | 'window';

/** Положение окна: угол — физические пиксели экрана, размер — логические. */
export interface WindowPlacement {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Как `DockSettings` в `src-tauri/src/settings.rs`. */
export interface DockSettings {
  mode: DockMode;
  /** Имя монитора (`\\.\DISPLAY2`); `null` — основной. */
  monitor: string | null;
  /** 320–640. */
  side_width: number;
  /** 480–1600. */
  top_width: number;
  /** 360–1200. */
  top_height: number;
  /** Задержка у края, 50–1000 мс; `null` — не выезжать от наведения. */
  hover_delay_ms: number | null;
  animate: boolean;
  indicator: boolean;
  window: WindowPlacement | null;
}

export interface Settings {
  /** 0 — не очищать буфер. */
  clipboard_clear_secs: number;
  theme: Theme;
  dock: DockSettings;
}

export const DEFAULT_DOCK: DockSettings = {
  mode: 'left',
  monitor: null,
  side_width: 400,
  top_width: 720,
  top_height: 520,
  hover_delay_ms: 200,
  animate: true,
  indicator: false,
  window: null,
};

export const DEFAULT_SETTINGS: Settings = {
  clipboard_clear_secs: 20,
  theme: 'system',
  dock: DEFAULT_DOCK,
};

export interface DockState {
  visible: boolean;
  pinned: boolean;
  mode: DockMode;
}

export interface MonitorInfo {
  id: string;
  /** Номер как в параметрах экрана Windows. */
  number: number;
  primary: boolean;
  /** Физические пиксели. */
  width: number;
  height: number;
  /** Масштаб, %. */
  scale: number;
}

export interface AppInfo {
  name: string;
  version: string;
  /** Обновления ставятся сами. В тестовых сборках — нет. */
  auto_update: boolean;
}

/** Цвета акцента Windows (`#rrggbb`): на светлом фоне темнее, на тёмном светлее. */
export interface Accent {
  light: string;
  dark: string;
}

export interface Appearance {
  /** Под окном фон Mica: интерфейс рисует полупрозрачные слои вместо сплошного фона. */
  mica: boolean;
  /** `null` — стандартный синий. */
  accent: Accent | null;
}

export type UpdateCheck =
  | { status: 'disabled' }
  | { status: 'up_to_date' }
  | { status: 'busy' }
  | { status: 'ready'; version: string };

/** Пустые секреты для новой записи. */
export function emptySecret(): ItemSecret {
  return { password: '', totp: '', notes: '', fields: [] };
}
