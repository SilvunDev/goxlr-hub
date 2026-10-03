// The only place where the interface talks to the Rust side.
import { invoke, isTauri } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  PROFILE_ERRORS,
  type Intent,
  type ProfileCommand,
  type ProfileError,
  type Snapshot,
} from './device';
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
 * Asks the Rust side to save, load or change a profile. Returns nothing when
 * it was done, or why it was not.
 */
export async function runProfileCommand(command: ProfileCommand): Promise<ProfileError | null> {
  // Outside Tauri there is nowhere to keep a profile.
  if (!isTauri()) return 'storage';
  try {
    await invoke('profile_command', { command });
    return null;
  } catch (error) {
    const known = PROFILE_ERRORS.find((code) => code === error);
    if (!known) console.error('Could not reach the profiles', error);
    return known ?? 'storage';
  }
}

/** How the app starts with the computer. */
export interface Startup {
  /** The computer starts the app. */
  enabled: boolean;
  /** Started by the computer, the app stays in the system tray. */
  hidden: boolean;
}

/** Nothing when the Rust side cannot tell. */
export async function getStartup(): Promise<Startup | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<Startup>('startup');
  } catch (error) {
    console.error('Could not read the startup setting', error);
    return null;
  }
}

/** Returns how the app starts now, or nothing when the change failed. */
export async function setStartup(startup: Startup): Promise<Startup | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<Startup>('set_startup', { ...startup });
  } catch (error) {
    console.error('Could not change the startup setting', error);
    return null;
  }
}

/** Quits the app for good. */
export async function quit(): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke('quit');
  } catch (error) {
    console.error('Could not quit', error);
  }
}

/** Calls `handler` each time the Rust side sends `event`. Returns a function that stops listening. */
function on<T>(event: string, handler: (payload: T) => void): () => void {
  // Outside Tauri there is nobody to hear from.
  if (!isTauri()) return () => {};

  let stopped = false;
  let unlisten: (() => void) | undefined;
  listen<T>(event, (received) => handler(received.payload))
    .then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    })
    .catch((error) => console.error(`Could not listen to ${event}`, error));

  return () => {
    stopped = true;
    unlisten?.();
  };
}

/**
 * Calls `handler` each time the Rust side reports the state of the device.
 * Returns a function that stops listening.
 */
export function onDeviceState(handler: (snapshot: Snapshot) => void): () => void {
  return on<Snapshot>('device-state', handler);
}

/**
 * Calls `handler` when the user asks to quit while something is not saved.
 * Returns a function that stops listening.
 */
export function onQuitRequested(handler: () => void): () => void {
  return on<unknown>('quit-requested', () => handler());
}
