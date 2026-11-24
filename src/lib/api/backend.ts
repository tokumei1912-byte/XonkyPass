import type {
  AppInfo,
  Copied,
  CopyTarget,
  ItemId,
  ItemList,
  ItemMeta,
  ItemSecret,
  ItemSummary,
  PassphraseOptions,
  PasswordOptions,
  Scope,
  Settings,
  Strength,
  VaultStatus,
} from './types';

export type Unsubscribe = () => void;

/**
 * Всё, что интерфейс может попросить у приложения. Две реализации: настоящая через
 * IPC Tauri (`tauri.ts`) и mock-бэкенд в памяти для разработки в браузере (`mock.ts`).
 * Ошибки — `ApiError` с кодом из Rust.
 */
export interface Backend {
  appInfo(): Promise<AppInfo>;

  vaultStatus(): Promise<VaultStatus>;
  /** Создаёт хранилище, открывает его и возвращает ключ восстановления. */
  vaultCreate(password: string): Promise<string>;
  vaultUnlock(password: string): Promise<void>;
  vaultUnlockRecovery(key: string): Promise<void>;
  vaultLock(): Promise<void>;
  vaultChangePassword(current: string, next: string): Promise<void>;
  /** Только после разблокировки ключом восстановления. */
  vaultResetPassword(next: string): Promise<void>;
  vaultRegenerateRecoveryKey(): Promise<string>;

  itemsList(scope: Scope): Promise<ItemList>;
  itemGet(id: ItemId): Promise<ItemSummary>;
  itemReveal(id: ItemId): Promise<ItemSecret>;
  itemCreate(meta: ItemMeta, secret: ItemSecret): Promise<ItemSummary>;
  /** `secret: null` — секреты не менялись. */
  itemUpdate(id: ItemId, meta: ItemMeta, secret: ItemSecret | null): Promise<ItemSummary>;
  itemTrash(id: ItemId): Promise<void>;
  itemRestore(id: ItemId): Promise<void>;
  itemPurge(id: ItemId): Promise<void>;
  /** Возвращает число удалённых записей. */
  trashEmpty(): Promise<number>;
  /** Копирует поле в буфер, не передавая его в интерфейс. */
  itemCopy(id: ItemId, target: CopyTarget): Promise<Copied>;

  generatePassword(options: PasswordOptions): Promise<string>;
  generatePassphrase(options: PassphraseOptions): Promise<string>;
  /** `inputs` — название, логин, сайт: пароль из них считается слабым. */
  passwordStrength(password: string, inputs: string[]): Promise<Strength>;

  /** Копирует текст, который уже есть в интерфейсе (сгенерированный пароль, ключ). */
  clipboardCopy(text: string): Promise<Copied>;
  /** Очищает буфер, если там наш секрет. */
  clipboardClear(): Promise<boolean>;

  settingsGet(): Promise<Settings>;
  settingsSet(settings: Settings): Promise<Settings>;

  /** Хранилище заблокировано (кнопкой, из трея, позже — по таймеру и Win+L). */
  onVaultLocked(handler: () => void): Promise<Unsubscribe>;
  /** Буфер обмена очищен по таймеру или при блокировке. */
  onClipboardCleared(handler: () => void): Promise<Unsubscribe>;
}
