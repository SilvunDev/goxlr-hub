// What the interface knows about the published versions of the app. Shared
// by the banner that tells about a new version and by the settings.
import {
  getPublishedVersions,
  getUpdateSettings,
  type PublishedVersion,
  type UpdateError,
  type UpdateSettings,
} from './backend';

/** The app lives in the tray for days: it looks again once a day. */
const CHECK_EVERY = 24 * 60 * 60 * 1000;

/** Nothing until the Rust side says which version runs. */
let settings = $state<UpdateSettings | null>(null);
/** Nothing until GitHub was asked and answered. */
let versions = $state<PublishedVersion[] | null>(null);
let checking = $state(false);
/** Why the last check asked by the user failed. */
let error = $state<UpdateError | null>(null);
/** The version the user chose to hear about later. */
let postponed = $state<string | null>(null);

export const updates = {
  get settings() {
    return settings;
  },
  set settings(now: UpdateSettings | null) {
    settings = now;
  },
  get versions() {
    return versions;
  },
  get checking() {
    return checking;
  },
  get error() {
    return error;
  },
  /** The latest version, when it came out after the one running. */
  get latest(): PublishedVersion | null {
    const latest = versions?.[0];
    return latest?.newer ? latest : null;
  },
  /** The new version to tell about, unless the user said later. */
  get announced(): PublishedVersion | null {
    const latest = this.latest;
    return latest && latest.version !== postponed ? latest : null;
  },
  postpone() {
    postponed = this.latest?.version ?? null;
  },
  /**
   * Asks GitHub for the published versions. A check the app makes by itself
   * fails in silence: only the user who asked is told.
   */
  async check(byUser: boolean) {
    if (checking) return;
    checking = true;
    const found = await getPublishedVersions();
    checking = false;
    if (Array.isArray(found)) {
      versions = found;
      error = null;
    } else if (byUser) {
      error = found;
    }
  },
  /**
   * Reads the settings, then looks for a new version now and once a day, as
   * long as the user lets the app do so. Returns a function that stops it.
   */
  start(): () => void {
    const look = () => {
      if (settings?.automatic) void this.check(false);
    };
    void getUpdateSettings().then((now) => {
      settings = now;
      look();
    });
    const timer = setInterval(look, CHECK_EVERY);
    return () => clearInterval(timer);
  },
  /** Forgets everything. For the tests. */
  reset() {
    settings = null;
    versions = null;
    checking = false;
    error = null;
    postponed = null;
  },
};
