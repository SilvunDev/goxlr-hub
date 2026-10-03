import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import App from './App.svelte';
import { i18n } from './lib/i18n/index.svelte';

function menuLabels(): string[] {
  const nav = screen.getByRole('navigation');
  return within(nav)
    .getAllByRole('button')
    .map((button) => button.textContent?.trim() ?? '');
}

describe('App', () => {
  beforeEach(() => {
    i18n.setLocale('en');
  });

  it('lists every section in order, then settings', () => {
    render(App);
    expect(menuLabels()).toEqual([
      'Mixer',
      'Microphone',
      'Channels',
      'Routing',
      'Controls',
      'Lighting',
      'Plugins',
      'Settings',
    ]);
  });

  it('opens on the mixer, marked as coming soon', () => {
    render(App);
    expect(screen.getByRole('button', { name: 'Mixer' }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Mixer');
    expect(screen.getByText('Coming soon')).toBeTruthy();
  });

  it('switches the whole interface to French from settings', async () => {
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
    await fireEvent.click(screen.getByRole('radio', { name: 'Français' }));

    expect(menuLabels()).toEqual([
      'Table',
      'Micro',
      'Pistes audio',
      'Routage',
      'Touches',
      'Éclairage',
      'Plugins',
      'Réglages',
    ]);
    expect(document.documentElement.lang).toBe('fr');
    expect(localStorage.getItem('goxlr-hub.locale')).toBe('fr');
  });

  it('shows the unofficial notice', () => {
    render(App);
    expect(screen.getByText(/not affiliated with TC-Helicon/i)).toBeTruthy();
  });
});
