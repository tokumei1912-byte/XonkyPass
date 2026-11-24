// Mock-бэкенд: хранилище в памяти для разработки интерфейса в браузере (`npm run dev`).
// Повторяет поведение Rust там, где это видно интерфейсу: коды ошибок, нормализацию
// записей, корзину, таймер буфера. Шифрования нет. В сборку приложения не попадает.
//
// Сценарий выбирается параметром адреса: `?mock=new` (первый запуск),
// `?mock=locked` (по умолчанию) или `?mock=open`. Мастер-пароль демо-хранилища — `demo`.

import type { Backend, Unsubscribe } from './backend';
import { ApiError } from './errors';
import type {
  Copied,
  CopyTarget,
  ItemId,
  ItemMeta,
  ItemSecret,
  ItemSummary,
  PassphraseOptions,
  PasswordOptions,
  Settings,
  Strength,
  UnlockMethod,
} from './types';
import { DEFAULT_SETTINGS, PASSPHRASE_WORDS, PASSWORD_LENGTH } from './types';

export type MockScenario = 'new' | 'locked' | 'open';

export const MOCK_PASSWORD = 'demo';

/** Время «вывода ключа», чтобы были видны состояния загрузки. */
const UNLOCK_DELAY_MS = 500;
const CREATE_DELAY_MS = 1200;
const CALL_DELAY_MS = 30;

const LOCKED_EVENT = 'locked';
const CLEARED_EVENT = 'cleared';
const SETTINGS_EVENT = 'settings';

interface StoredItem {
  summary: ItemSummary;
  secret: ItemSecret;
}

export function createMockBackend(scenario: MockScenario = 'locked'): Backend {
  const events = new EventTarget();
  let exists = scenario !== 'new';
  let password = MOCK_PASSWORD;
  let recoveryKey = exists ? randomRecoveryKey() : '';
  let unlockedWith: UnlockMethod | null = scenario === 'open' ? 'password' : null;
  let settings: Settings = { ...DEFAULT_SETTINGS };
  const items = new Map<ItemId, StoredItem>(
    exists ? demoItems().map((i) => [i.summary.id, i]) : [],
  );
  let clipboardTimer: ReturnType<typeof setTimeout> | undefined;
  /** В буфере наш секрет: его очистят таймер, блокировка или кнопка. */
  let clipboardOurs = false;

  const requireUnlocked = () => {
    if (!unlockedWith) throw new ApiError('locked', 'хранилище заблокировано');
  };
  const find = (id: ItemId): StoredItem => {
    requireUnlocked();
    const item = items.get(id);
    if (!item) throw new ApiError('item_not_found', 'запись не найдена');
    return item;
  };
  const on = (name: string, handler: () => void): Promise<Unsubscribe> => {
    const listener = () => handler();
    events.addEventListener(name, listener);
    return Promise.resolve(() => events.removeEventListener(name, listener));
  };
  const clearClipboard = (): boolean => {
    clearTimeout(clipboardTimer);
    if (!clipboardOurs) return false;
    clipboardOurs = false;
    events.dispatchEvent(new Event(CLEARED_EVENT));
    return true;
  };
  const copy = (what: string): Copied => {
    // Сам текст не выводим: в журнале браузера ему не место даже в разработке.
    console.info(`[mock] в буфер скопировано: ${what}`);
    clearTimeout(clipboardTimer);
    clipboardOurs = true;
    const secs = settings.clipboard_clear_secs;
    if (secs > 0) clipboardTimer = setTimeout(clearClipboard, secs * 1000);
    return { clear_after_secs: secs };
  };

  return {
    async appInfo() {
      return { name: 'XonkyPass', version: 'dev (mock)', auto_update: false };
    },

    async appearance() {
      // `?accent=#rrggbb` — проверить чужой акцент без Windows.
      const accent = new URLSearchParams(window.location.search).get('accent');
      return { mica: false, accent: accent ? { light: accent, dark: accent } : null };
    },

    async updateCheck() {
      return { status: 'disabled' };
    },

    async updateInstall() {},

    async openUrl(url) {
      console.info(`[mock] открыть сайт: ${url}`);
    },

    async revealVault() {
      console.info('[mock] показать файл хранилища');
    },

    async recoveryKeySave(key) {
      if (normalizeRecoveryKey(key).length !== 28) {
        throw new ApiError('invalid_recovery_key', 'неверный формат ключа');
      }
      console.info('[mock] ключ восстановления сохранён в файл');
      return true;
    },

    async vaultStatus() {
      await sleep(CALL_DELAY_MS);
      return {
        exists,
        unlocked: unlockedWith !== null,
        unlocked_with: unlockedWith,
        path: '%APPDATA%\\XonkyPass\\vault.xpv',
      };
    },

    async vaultCreate(newPassword) {
      if (exists) throw new ApiError('already_exists', 'хранилище уже существует');
      requirePassword(newPassword);
      await sleep(CREATE_DELAY_MS);
      exists = true;
      password = newPassword;
      recoveryKey = randomRecoveryKey();
      unlockedWith = 'password';
      return recoveryKey;
    },

    async vaultUnlock(attempt) {
      if (!exists) throw new ApiError('vault_not_found', 'хранилище не найдено');
      await sleep(UNLOCK_DELAY_MS);
      if (attempt !== password) throw new ApiError('wrong_password', 'неверный пароль');
      unlockedWith = 'password';
    },

    async vaultUnlockRecovery(key) {
      const normalized = normalizeRecoveryKey(key);
      if (normalized.length !== 28 || [...normalized].some((c) => !CROCKFORD.includes(c))) {
        throw new ApiError('invalid_recovery_key', 'неверный формат ключа');
      }
      await sleep(UNLOCK_DELAY_MS);
      if (normalized !== normalizeRecoveryKey(recoveryKey)) {
        throw new ApiError('wrong_recovery_key', 'ключ не подходит');
      }
      unlockedWith = 'recovery_key';
    },

    async vaultLock() {
      unlockedWith = null;
      clearClipboard();
      events.dispatchEvent(new Event(LOCKED_EVENT));
    },

    async vaultChangePassword(current, next) {
      requireUnlocked();
      requirePassword(next);
      await sleep(UNLOCK_DELAY_MS * 2);
      if (current !== password) throw new ApiError('wrong_password', 'неверный пароль');
      password = next;
    },

    async vaultResetPassword(next) {
      requireUnlocked();
      if (unlockedWith !== 'recovery_key') {
        throw new ApiError('not_allowed', 'только после входа по ключу восстановления');
      }
      requirePassword(next);
      await sleep(UNLOCK_DELAY_MS);
      password = next;
      unlockedWith = 'password';
    },

    async vaultRegenerateRecoveryKey() {
      requireUnlocked();
      await sleep(UNLOCK_DELAY_MS);
      recoveryKey = randomRecoveryKey();
      return recoveryKey;
    },

    async itemsList(scope) {
      requireUnlocked();
      await sleep(CALL_DELAY_MS);
      const inTrash = scope === 'trash';
      const list = [...items.values()]
        .map((i) => i.summary)
        .filter((s) => (s.deleted_at !== null) === inTrash)
        .sort((a, b) => b.updated_at - a.updated_at);
      return { items: structuredClone(list), damaged: [] };
    },

    async itemGet(id) {
      return structuredClone(find(id).summary);
    },

    async itemReveal(id) {
      return structuredClone(find(id).secret);
    },

    async itemCreate(meta, secret) {
      requireUnlocked();
      const cleanSecret = normalizeSecret(secret);
      const cleanMeta = normalizeMeta(meta, cleanSecret);
      const now = Date.now();
      const summary: ItemSummary = {
        id: crypto.randomUUID(),
        meta: cleanMeta,
        created_at: now,
        updated_at: now,
        deleted_at: null,
        revision: 1,
        use_count: 0,
        last_used_at: null,
      };
      items.set(summary.id, { summary, secret: structuredClone(cleanSecret) });
      return structuredClone(summary);
    },

    async itemUpdate(id, meta, secret) {
      const item = find(id);
      if (item.summary.meta.kind !== meta.kind) {
        throw new ApiError('invalid', 'нельзя изменить тип записи');
      }
      const nextSecret = secret ? normalizeSecret(secret) : item.secret;
      item.summary.meta = normalizeMeta(meta, nextSecret);
      item.secret = structuredClone(nextSecret);
      item.summary.updated_at = Date.now();
      item.summary.revision += 1;
      return structuredClone(item.summary);
    },

    async itemTrash(id) {
      const item = find(id);
      item.summary.deleted_at ??= Date.now();
    },

    async itemRestore(id) {
      const item = find(id);
      if (item.summary.deleted_at === null) throw new ApiError('not_in_trash', 'не в корзине');
      item.summary.deleted_at = null;
    },

    async itemPurge(id) {
      const item = find(id);
      if (item.summary.deleted_at === null) throw new ApiError('not_in_trash', 'не в корзине');
      items.delete(id);
    },

    async trashEmpty() {
      requireUnlocked();
      let removed = 0;
      for (const [id, item] of items) {
        if (item.summary.deleted_at !== null) {
          items.delete(id);
          removed += 1;
        }
      }
      return removed;
    },

    async itemCopy(id, target) {
      const item = find(id);
      const text = fieldText(item, target);
      if (!text) throw new ApiError('empty_field', 'поле пустое');
      if (target.field === 'username' || target.field === 'password') {
        item.summary.use_count += 1;
        item.summary.last_used_at = Date.now();
      }
      return copy(`${item.summary.meta.title}: ${target.field}`);
    },

    async generatePassword(options) {
      return generatePassword(options);
    },

    async generatePassphrase(options) {
      return generatePassphrase(options);
    },

    async passwordStrength(candidate, inputs) {
      return estimateStrength(candidate, inputs);
    },

    async clipboardCopy(text) {
      if (!text) throw new ApiError('empty_field', 'поле пустое');
      return copy(`текст (${text.length} симв.)`);
    },

    async clipboardClear() {
      return clearClipboard();
    },

    async settingsGet() {
      return { ...settings };
    },

    async settingsSet(next) {
      if (next.clipboard_clear_secs < 0 || next.clipboard_clear_secs > 600) {
        throw new ApiError('invalid', 'очистка буфера — не дольше 600 секунд');
      }
      settings = { ...next };
      events.dispatchEvent(new CustomEvent(SETTINGS_EVENT, { detail: { ...settings } }));
      return { ...settings };
    },

    onVaultLocked: (handler) => on(LOCKED_EVENT, handler),
    onClipboardCleared: (handler) => on(CLEARED_EVENT, handler),
    onSettingsChanged: (handler) => {
      const listener = (e: Event) => handler((e as CustomEvent<Settings>).detail);
      events.addEventListener(SETTINGS_EVENT, listener);
      return Promise.resolve(() => events.removeEventListener(SETTINGS_EVENT, listener));
    },
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function requirePassword(value: string): void {
  if (!value) throw new ApiError('invalid', 'пустой мастер-пароль');
}

function fieldText(item: StoredItem, target: CopyTarget): string {
  const { meta } = item.summary;
  switch (target.field) {
    case 'username':
      return meta.kind === 'login' ? meta.username : '';
    case 'body':
      return meta.kind === 'note' ? meta.body : '';
    case 'password':
      return item.secret.password;
    case 'notes':
      return item.secret.notes;
    case 'custom':
      return item.secret.fields[target.index]?.value ?? '';
  }
}

// --- Нормализация, как в `xonkypass_core::model` -------------------------------

/** Без пробелов по краям, пустых значений и повторов (без учёта регистра). */
function normalizeList(values: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of values) {
    const value = raw.trim();
    const key = value.toLowerCase();
    if (value && !seen.has(key)) {
      seen.add(key);
      result.push(value);
    }
  }
  return result;
}

/** Тег без `#` в начале, пробелы заменены на `-`. */
function normalizeTags(tags: string[]): string[] {
  return normalizeList(
    tags.map((tag) => tag.trim().replace(/^#+/, '').trim().split(/\s+/).join('-')),
  );
}

function normalizeMeta(meta: ItemMeta, secret: ItemSecret): ItemMeta {
  if (meta.kind === 'login') {
    const urls = normalizeList(meta.urls);
    const apps = normalizeList(meta.apps);
    // Запись, сохранённая с сайта или из приложения, называется по нему.
    const title = meta.title.trim() || urls[0] || apps[0];
    if (!title) throw new ApiError('invalid', 'пустое название');
    return {
      ...meta,
      title,
      username: meta.username.trim(),
      urls,
      apps,
      tags: normalizeTags(meta.tags),
      has_totp: secret.totp !== '',
    };
  }
  let title = meta.title.trim();
  if (!title) {
    // Быстрая заметка без названия называется по первой строке текста.
    const line = meta.body
      .split(/\r?\n/)
      .map((l) => l.trim().replace(/^#+/, '').trim())
      .find(Boolean);
    if (!line) throw new ApiError('invalid', 'пустая заметка');
    title = [...line].slice(0, 80).join('');
  }
  return { ...meta, title, tags: normalizeTags(meta.tags) };
}

function normalizeSecret(secret: ItemSecret): ItemSecret {
  const fields = secret.fields
    .filter((field) => field.name.trim() || field.value)
    .map((field) => {
      const name = field.name.trim();
      if (!name) throw new ApiError('invalid', 'у дополнительного поля нет названия');
      return { ...field, name };
    });
  return { ...secret, totp: secret.totp.trim(), fields };
}

// --- Генератор и оценка -----------------------------------------------------------

const LOWER = 'abcdefghijklmnopqrstuvwxyz';
const UPPER = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
const DIGITS = '0123456789';
const SYMBOLS = '!@#$%^&*()-_=+[]{};:,.?/';
const SIMILAR = new Set('0O1lI|');

function randomBelow(n: number): number {
  // Отбрасываем значения из «хвоста», чтобы распределение было равномерным.
  const limit = Math.floor(0x1_0000_0000 / n) * n;
  const buffer = new Uint32Array(1);
  for (;;) {
    crypto.getRandomValues(buffer);
    if (buffer[0]! < limit) return buffer[0]! % n;
  }
}

function pick(chars: string): string {
  return chars[randomBelow(chars.length)]!;
}

function generatePassword(options: PasswordOptions): string {
  const { min, max } = PASSWORD_LENGTH;
  if (options.length < min || options.length > max) {
    throw new ApiError('invalid', `длина должна быть от ${min} до ${max}`);
  }
  const filter = (set: string) =>
    options.avoid_similar ? [...set].filter((c) => !SIMILAR.has(c)).join('') : set;
  const sets = [
    options.lowercase && filter(LOWER),
    options.uppercase && filter(UPPER),
    options.digits && filter(DIGITS),
    options.symbols && filter(SYMBOLS),
  ].filter((s): s is string => Boolean(s));
  if (sets.length === 0) throw new ApiError('invalid', 'не выбран ни один набор символов');
  const all = sets.join('');
  // По символу из каждого набора, остальное — из всех, затем перемешиваем.
  const chars = sets.map(pick);
  while (chars.length < options.length) chars.push(pick(all));
  for (let i = chars.length - 1; i > 0; i -= 1) {
    const j = randomBelow(i + 1);
    [chars[i], chars[j]] = [chars[j]!, chars[i]!];
  }
  return chars.join('');
}

const MOCK_WORDS = [
  'anchor',
  'bonfire',
  'canyon',
  'dimmer',
  'ferment',
  'glider',
  'harbor',
  'ivory',
  'jigsaw',
  'kettle',
  'lantern',
  'meadow',
  'mutable',
  'nimbus',
  'orbit',
  'oxidize',
  'pastel',
  'pickle',
  'quench',
  'ribbon',
  'saddle',
  'thrive',
  'unsaid',
  'velvet',
  'walnut',
  'yonder',
  'zipper',
];

function generatePassphrase(options: PassphraseOptions): string {
  const { min, max } = PASSPHRASE_WORDS;
  if (options.words < min || options.words > max) {
    throw new ApiError('invalid', `длина должна быть от ${min} до ${max}`);
  }
  const words = Array.from({ length: options.words }, () => {
    const word = MOCK_WORDS[randomBelow(MOCK_WORDS.length)]!;
    return options.capitalize ? word[0]!.toUpperCase() + word.slice(1) : word;
  });
  if (options.number) {
    const i = randomBelow(words.length);
    words[i] += String(randomBelow(10));
  }
  return words.join(options.separator);
}

/** Грубая оценка вместо zxcvbn: размер алфавита и длина, минус слова из записи. */
function estimateStrength(password: string, inputs: string[]): Strength {
  if (!password) return { score: 0, guesses_log10: 0 };
  let alphabet = 0;
  if (/[a-z]/.test(password)) alphabet += 26;
  if (/[A-Z]/.test(password)) alphabet += 26;
  if (/\d/.test(password)) alphabet += 10;
  if (/[^a-zA-Z\d]/.test(password)) alphabet += 33;
  let guesses = Math.min(password.length, 100) * Math.log10(Math.max(alphabet, 10));
  const lower = password.toLowerCase();
  if (inputs.some((input) => input.length >= 3 && lower.includes(input.toLowerCase()))) {
    guesses /= 2;
  }
  const score: Strength['score'] =
    guesses < 3 ? 0 : guesses < 6 ? 1 : guesses < 8 ? 2 : guesses < 10 ? 3 : 4;
  return { score, guesses_log10: Math.round(guesses * 100) / 100 };
}

// --- Ключ восстановления ----------------------------------------------------------

const CROCKFORD = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

function randomRecoveryKey(): string {
  const chars = Array.from({ length: 28 }, () => pick(CROCKFORD));
  return chars.join('').replace(/(.{4})(?=.)/g, '$1-');
}

function normalizeRecoveryKey(key: string): string {
  return key.toUpperCase().replace(/[\s-]/g, '').replace(/O/g, '0').replace(/[IL]/g, '1');
}

// --- Демо-данные ------------------------------------------------------------------

function demoItems(): StoredItem[] {
  const day = 24 * 60 * 60 * 1000;
  const now = Date.now();
  let n = 0;
  const make = (
    meta: ItemMeta,
    secret: Partial<ItemSecret> = {},
    opts: { ageDays?: number; deletedDaysAgo?: number; uses?: number } = {},
  ): StoredItem => {
    n += 1;
    const updated = now - (opts.ageDays ?? n) * day;
    return {
      summary: {
        id: `0190a000-0000-7000-8000-${String(n).padStart(12, '0')}`,
        meta,
        created_at: updated - 30 * day,
        updated_at: updated,
        deleted_at: opts.deletedDaysAgo === undefined ? null : now - opts.deletedDaysAgo * day,
        revision: 1,
        use_count: opts.uses ?? 0,
        last_used_at: opts.uses ? updated : null,
      },
      secret: { password: '', totp: '', notes: '', fields: [], ...secret },
    };
  };
  const login = (
    title: string,
    username: string,
    urls: string[],
    tags: string[] = [],
    extra: Partial<Omit<ItemMeta & { kind: 'login' }, 'kind'>> = {},
  ): ItemMeta => ({
    kind: 'login',
    title,
    username,
    urls,
    apps: [],
    tags,
    favorite: false,
    has_totp: false,
    ...extra,
  });
  const note = (title: string, body: string, tags: string[] = []): ItemMeta => ({
    kind: 'note',
    title,
    body,
    tags,
    favorite: false,
  });

  return [
    make(
      login('GitHub', 'tokumei@example.com', ['github.com', 'gist.github.com'], ['dev', 'работа'], {
        apps: ['GitHubDesktop.exe'],
        favorite: true,
      }),
      {
        password: 'Tq7#vR2m!xL9pW4z@Kd8',
        notes: 'Резервные коды 2FA — в заметке «GitHub: коды восстановления».',
        fields: [{ name: 'PIN токена', value: '482913', hidden: true }],
      },
      { ageDays: 1, uses: 42 },
    ),
    make(login('Яндекс ID', 'me@yandex.ru', ['passport.yandex.ru']), { password: 'kedr-lampa-91' }),
    make(login('Госуслуги', '+7 999 000-00-00', ['gosuslugi.ru'], ['дом']), {
      password: 'Gs!2026-vesna',
    }),
    make(login('Сбербанк Онлайн', 'ivan.petrov', ['online.sberbank.ru'], ['банки']), {
      password: 'S8b#nk-Online-77',
    }),
    make(
      note(
        'Wi-Fi и роутер',
        '## Домашняя сеть\n- Сеть: `Home_5G`\n- Панель роутера: `192.168.0.1`, логин `admin`\n- Пароль от Wi-Fi — на наклейке под роутером\n\n## Гостевая сеть\nСеть `Guest`, пароль меняю раз в месяц.\n\n- [x] Обновить прошивку\n- [ ] Сменить пароль гостевой сети',
        ['дом'],
      ),
    ),
    make(login('Steam', 'gamer_2026', [], ['игры'], { apps: ['steam.exe'] }), {
      password: 'pixel-Quest-2026',
    }),
    make(login('Рабочий VPN', 'i.petrov', ['vpn.corp.example'], ['работа']), {
      password: 'Vpn$ecure-0451',
    }),
    make(login('Telegram', '+7 999 000-00-00', [], [], { apps: ['Telegram.exe'] })),
    make(login('Ozon', 'ivan.petrov@example.com', ['ozon.ru']), { password: 'ozon-Basket-12' }),
    make(login('GitLab (работа)', 'i.petrov', ['gitlab.corp.example'], ['работа', 'dev']), {
      password: 'GL-w0rk!pass',
    }),
    make(note('SSH-ключи', 'Ключ `id_ed25519` добавлен в GitHub и на сервер сборки.', ['dev'])),
    make(
      login('Skype', 'ivan.petrov', ['skype.com']),
      { password: 'old-skype-pass' },
      {
        deletedDaysAgo: 2,
      },
    ),
    make(note('Старый список дел', '- [ ] Разобрать почту'), {}, { deletedDaysAgo: 19 }),
  ];
}
