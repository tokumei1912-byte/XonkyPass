// Тема, акцент Windows и фон Mica: атрибуты и переменные на <html> для `app.css`.

import { api, type Theme } from './api';

const root = document.documentElement;
const systemDark = window.matchMedia('(prefers-color-scheme: dark)');
let theme: Theme = 'system';

function applyTheme(): void {
  const dark = theme === 'dark' || (theme === 'system' && systemDark.matches);
  root.dataset.theme = dark ? 'dark' : 'light';
}

export function setTheme(next: Theme): void {
  theme = next;
  applyTheme();
}

/** Фон Mica и акцент. Акцент пользователь может сменить в любой момент — перечитываем. */
export async function refreshAppearance(): Promise<void> {
  try {
    const { mica, accent } = await api.appearance();
    root.toggleAttribute('data-mica', mica);
    setColor('--accent-light', accent?.light);
    setColor('--accent-dark', accent?.dark);
  } catch (error) {
    console.warn('оформление окна недоступно', error);
  }
}

function setColor(name: string, value: string | undefined): void {
  if (value && /^#[0-9a-f]{6}$/i.test(value)) root.style.setProperty(name, value);
  else root.style.removeProperty(name);
}

/** Один раз при запуске окна. */
export function initAppearance(): void {
  applyTheme();
  systemDark.addEventListener('change', applyTheme);
  window.addEventListener('focus', () => void refreshAppearance());
  void refreshAppearance();
  api
    .settingsGet()
    .then((settings) => setTheme(settings.theme))
    .catch((error: unknown) => console.warn('настройки недоступны', error));
  void api.onSettingsChanged((settings) => setTheme(settings.theme));
}
