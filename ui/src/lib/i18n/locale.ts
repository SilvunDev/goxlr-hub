export const locales = ['en', 'fr'] as const;

export type Locale = (typeof locales)[number];

/** Each language is listed under its own name, whatever the current one is. */
export const localeNames: Record<Locale, string> = {
  en: 'English',
  fr: 'Français',
};

/** Reads a language tag such as `fr-FR`; null when the language is not supported. */
export function normalizeLocale(tag: string | null | undefined): Locale | null {
  const language = tag?.split(/[-_.]/)[0]?.toLowerCase();
  return locales.find((locale) => locale === language) ?? null;
}

/** The saved choice wins, then the first supported system language, then English. */
export function pickInitialLocale(stored: string | null, preferred: readonly string[]): Locale {
  const saved = normalizeLocale(stored);
  if (saved) return saved;
  for (const tag of preferred) {
    const locale = normalizeLocale(tag);
    if (locale) return locale;
  }
  return 'en';
}
