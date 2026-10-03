// The only place where the interface talks to the Rust side.
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Locale } from './i18n/locale';

/** Tells the Rust side which language to use for the tray menu. */
export async function syncLocale(locale: Locale): Promise<void> {
  // Outside Tauri (tests, plain browser) there is nobody to tell.
  if (!isTauri()) return;
  try {
    await invoke('set_locale', { tag: locale });
  } catch (error) {
    console.error('Could not update the tray menu language', error);
  }
}
