import { describe, expect, it } from 'vitest';
import { normalizeLocale, pickInitialLocale } from './locale';

describe('normalizeLocale', () => {
  it('maps regional tags to a supported locale', () => {
    expect(normalizeLocale('fr-FR')).toBe('fr');
    expect(normalizeLocale('FR_ca')).toBe('fr');
    expect(normalizeLocale('en-GB')).toBe('en');
  });

  it('rejects unsupported or broken tags', () => {
    for (const tag of ['de-DE', '', '  ', 'french', null, undefined]) {
      expect(normalizeLocale(tag)).toBeNull();
    }
  });
});

describe('pickInitialLocale', () => {
  it('prefers the stored choice', () => {
    expect(pickInitialLocale('fr', ['en-US'])).toBe('fr');
  });

  it('ignores a corrupted stored value', () => {
    expect(pickInitialLocale('{"oops":1}', ['fr-FR'])).toBe('fr');
  });

  it('takes the first supported system language', () => {
    expect(pickInitialLocale(null, ['de-DE', 'fr-CH', 'en'])).toBe('fr');
  });

  it('falls back to English', () => {
    expect(pickInitialLocale(null, ['de-DE'])).toBe('en');
    expect(pickInitialLocale(null, [])).toBe('en');
  });
});
