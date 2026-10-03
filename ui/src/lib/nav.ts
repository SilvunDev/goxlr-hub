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

export type SectionId = (typeof sections)[number] | 'settings';
