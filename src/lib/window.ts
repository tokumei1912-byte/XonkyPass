import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

export type WindowLabel = 'panel' | 'palette';

/**
 * Имя текущего окна. Вне Tauri (разработка в браузере) берётся из `?window=…`.
 */
export function currentWindowLabel(): WindowLabel {
  const label = isTauri()
    ? getCurrentWindow().label
    : new URLSearchParams(window.location.search).get('window');
  return label === 'palette' ? 'palette' : 'panel';
}

/** Спрятать текущее окно (не закрывая его). */
export async function hideCurrentWindow(): Promise<void> {
  if (isTauri()) await getCurrentWindow().hide();
}
