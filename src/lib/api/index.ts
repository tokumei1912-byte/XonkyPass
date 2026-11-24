import { isTauri } from '@tauri-apps/api/core';
import type { Backend } from './backend';
import { createMockBackend, type MockScenario } from './mock';
import { tauriBackend } from './tauri';

export type { Backend, Unsubscribe } from './backend';
export { ApiError, errorText, toApiError } from './errors';
export * from './types';

function mockScenario(): MockScenario {
  const value = new URLSearchParams(window.location.search).get('mock');
  return value === 'new' || value === 'open' ? value : 'locked';
}

/**
 * Бэкенд интерфейса. В приложении — команды Rust. В браузере при `npm run dev` —
 * mock-бэкенд в памяти; в сборку он не попадает: `import.meta.env.DEV` там `false`.
 */
export const api: Backend =
  import.meta.env.DEV && !isTauri() ? createMockBackend(mockScenario()) : tauriBackend;
