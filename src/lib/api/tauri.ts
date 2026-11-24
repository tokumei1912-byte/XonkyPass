import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Backend } from './backend';
import { toApiError } from './errors';

/** Имена событий — как в `src-tauri/src/events.rs`. */
const EVENTS = {
  vaultLocked: 'vault://locked',
  clipboardCleared: 'clipboard://cleared',
} as const;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toApiError(error);
  }
}

/** Настоящий бэкенд: команды Rust через IPC Tauri. */
export const tauriBackend: Backend = {
  appInfo: () => call('app_info'),

  vaultStatus: () => call('vault_status'),
  vaultCreate: (password) => call('vault_create', { password }),
  vaultUnlock: (password) => call('vault_unlock', { password }),
  vaultUnlockRecovery: (key) => call('vault_unlock_recovery', { key }),
  vaultLock: () => call('vault_lock'),
  vaultChangePassword: (current, next) => call('vault_change_password', { current, new: next }),
  vaultResetPassword: (next) => call('vault_reset_password', { new: next }),
  vaultRegenerateRecoveryKey: () => call('vault_regenerate_recovery_key'),

  itemsList: (scope) => call('items_list', { scope }),
  itemGet: (id) => call('item_get', { id }),
  itemReveal: (id) => call('item_reveal', { id }),
  itemCreate: (meta, secret) => call('item_create', { meta, secret }),
  itemUpdate: (id, meta, secret) => call('item_update', { id, meta, secret }),
  itemTrash: (id) => call('item_trash', { id }),
  itemRestore: (id) => call('item_restore', { id }),
  itemPurge: (id) => call('item_purge', { id }),
  trashEmpty: () => call('trash_empty'),
  itemCopy: (id, target) => call('item_copy', { id, target }),

  generatePassword: (options) => call('generate_password', { options }),
  generatePassphrase: (options) => call('generate_passphrase', { options }),
  passwordStrength: (password, inputs) => call('password_strength', { password, inputs }),

  clipboardCopy: (text) => call('clipboard_copy', { text }),
  clipboardClear: () => call('clipboard_clear'),

  settingsGet: () => call('settings_get'),
  settingsSet: (settings) => call('settings_set', { settings }),

  onVaultLocked: (handler) => listen(EVENTS.vaultLocked, () => handler()),
  onClipboardCleared: (handler) => listen(EVENTS.clipboardCleared, () => handler()),
};
