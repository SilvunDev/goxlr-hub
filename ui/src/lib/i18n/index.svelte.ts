import { syncLocale } from '../backend';
import { en, type Messages } from './en';
import { fr } from './fr';
import { pickInitialLocale, type Locale } from './locale';

const STORAGE_KEY = 'goxlr-hub.locale';

const catalogs: Record<Locale, Messages> = { en, fr };

function readStored(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function apply(locale: Locale) {
  document.documentElement.lang = locale;
  void syncLocale(locale);
}

let current = $state<Locale>(pickInitialLocale(readStored(), navigator.languages));
apply(current);

export const i18n = {
  get locale(): Locale {
    return current;
  },
  /** The texts of the current language. */
  get t(): Messages {
    return catalogs[current];
  },
  setLocale(locale: Locale) {
    current = locale;
    try {
      localStorage.setItem(STORAGE_KEY, locale);
    } catch {
      // Storage can be unavailable; the choice then lasts until the app closes.
    }
    apply(locale);
  },
};
