// Настройки генератора паролей: последние выбранные запоминаются в браузерном
// хранилище окна (это не секрет — только длина и наборы символов).

import {
  api,
  DEFAULT_PASSPHRASE_OPTIONS,
  DEFAULT_PASSWORD_OPTIONS,
  type PassphraseOptions,
  type PasswordOptions,
} from './api';

export type GeneratorMode = 'password' | 'passphrase';

export interface GeneratorOptions {
  mode: GeneratorMode;
  password: PasswordOptions;
  passphrase: PassphraseOptions;
}

const STORAGE_KEY = 'xonkypass.generator';

export function loadGeneratorOptions(): GeneratorOptions {
  const defaults: GeneratorOptions = {
    mode: 'password',
    password: { ...DEFAULT_PASSWORD_OPTIONS },
    passphrase: { ...DEFAULT_PASSPHRASE_OPTIONS },
  };
  try {
    const saved = JSON.parse(
      localStorage.getItem(STORAGE_KEY) ?? 'null',
    ) as Partial<GeneratorOptions> | null;
    if (!saved) return defaults;
    return {
      mode: saved.mode === 'passphrase' ? 'passphrase' : 'password',
      password: { ...defaults.password, ...saved.password },
      passphrase: { ...defaults.passphrase, ...saved.passphrase },
    };
  } catch {
    return defaults;
  }
}

export function saveGeneratorOptions(options: GeneratorOptions): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(options));
  } catch {
    // Хранилище браузера недоступно — просто не запоминаем.
  }
}

export function generate(options: GeneratorOptions): Promise<string> {
  return options.mode === 'passphrase'
    ? api.generatePassphrase(options.passphrase)
    : api.generatePassword(options.password);
}
