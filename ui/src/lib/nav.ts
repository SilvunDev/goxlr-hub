/** Menu sections, in display order. */
export const sections = [
  'mixer',
  'mic',
  'channels',
  'routing',
  'controls',
  'lighting',
  'plugins',
] as const;

/** Profiles are opened from the header, settings from the foot of the menu. */
export type SectionId = (typeof sections)[number] | 'profiles' | 'settings';
