// The only place where the interface talks to the Rust side.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Intent, Snapshot } from './device';
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

/** Asks the Rust side to change something on the device shown. */
export async function sendIntent(intent: Intent): Promise<void> {
  // Outside Tauri there is no device to change.
  if (!isTauri()) return;
  try {
    await invoke('mixer_intent', { intent });
  } catch (error) {
    console.error('Could not reach the device', error);
  }
}

/**
 * Calls `handler` each time the Rust side reports the state of the device.
 * Returns a function that stops listening.
 */
export function onDeviceState(handler: (snapshot: Snapshot) => void): () => void {
  // Outside Tauri there is no device to hear from.
  if (!isTauri()) return () => {};

  let stopped = false;
  let unlisten: (() => void) | undefined;
  listen<Snapshot>('device-state', (event) => handler(event.payload))
    .then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    })
    .catch((error) => console.error('Could not listen to the device', error));

  return () => {
    stopped = true;
    unlisten?.();
  };
}
