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

/** What the interface needs to know about updates before asking GitHub. */
export interface UpdateSettings {
  /** The version running. */
  version: string;
  /** The app asks GitHub by itself whether a new version is out. */
  automatic: boolean;
  /** The app can install a version by itself. Elsewhere it opens its page. */
  installs: boolean;
}

/** A published version of the app. */
export interface PublishedVersion {
  version: string;
  /** When it was published, as GitHub writes dates. */
  published: string;
  /** It is the one running. */
  current: boolean;
  /** It came out after the one running. */
  newer: boolean;
  /** The app can install it by itself. */
  installable: boolean;
}

export const UPDATE_ERRORS = [
  'offline',
  'unexpected',
  'notFound',
  'corrupt',
  'unsaved',
  'storage',
  'unsupported',
] as const;

/** Why the versions could not be listed, or one could not be installed. */
export type UpdateError = (typeof UPDATE_ERRORS)[number];

function updateError(error: unknown): UpdateError {
  const known = UPDATE_ERRORS.find((code) => code === error);
  if (!known) console.error('Could not reach the updates', error);
  return known ?? 'unexpected';
}

/** Nothing when the Rust side cannot tell. */
export async function getUpdateSettings(): Promise<UpdateSettings | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<UpdateSettings>('update_settings');
  } catch (error) {
    console.error('Could not read the update settings', error);
    return null;
  }
}

/** Returns the settings as they are now, or nothing when the change failed. */
export async function setAutomaticUpdates(automatic: boolean): Promise<UpdateSettings | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<UpdateSettings>('set_automatic_updates', { automatic });
  } catch (error) {
    console.error('Could not change the update settings', error);
    return null;
  }
}

/** The published versions, latest first, or why they could not be listed. */
export async function getPublishedVersions(): Promise<PublishedVersion[] | UpdateError> {
  if (!isTauri()) return 'offline';
  try {
    return await invoke<PublishedVersion[]>('published_versions');
  } catch (error) {
    return updateError(error);
  }
}

/**
 * Downloads and starts the installer of a published version. The app quits
 * when it worked; otherwise returns why it did not.
 */
export async function installVersion(version: string): Promise<UpdateError | null> {
  if (!isTauri()) return 'unsupported';
  try {
    await invoke('install_version', { version });
    return null;
  } catch (error) {
    return updateError(error);
  }
}

/** Opens the page of a published version in the browser. */
export async function openRelease(version: string): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke('open_release', { version });
  } catch (error) {
    console.error('Could not open the page of the version', error);
  }
}

/** Opens the folder the profiles are backed up in. */
export async function openBackups(): Promise<void> {
  if (!isTauri()) return;
  try {
    await invoke('open_backups');
  } catch (error) {
    console.error('Could not open the backups', error);
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
