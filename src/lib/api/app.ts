import { invoke, isTauri } from '@tauri-apps/api/core';

export interface AppInfo {
  name: string;
  version: string;
}

/** Название и версия приложения (проверка, что IPC с Rust работает). */
export async function getAppInfo(): Promise<AppInfo> {
  if (!isTauri()) return { name: 'XonkyPass', version: 'dev' };
  return invoke<AppInfo>('app_info');
}
