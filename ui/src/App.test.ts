import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import type { ChannelId, FaderId, Snapshot } from './lib/device';
import { i18n } from './lib/i18n/index.svelte';

// Stands in for the Rust side: `feed.push` plays the device reporting its state.
const feed = vi.hoisted(() => ({ push: (_snapshot: unknown) => {} }));

vi.mock('./lib/backend', () => ({
  syncLocale: async () => {},
  onDeviceState: (handler: (snapshot: unknown) => void) => {
    feed.push = handler;
    return () => {};
  },
}));

function snapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  const fader = (id: FaderId, channel: ChannelId, volume: number, muted = false) => ({
    fader: id,
    channel,
    volume,
    muted,
  });
  return {
    connection: { state: 'demo' },
    device: { kind: 'virtual', firmware: '1.4.3.110', serial: 'VIRTUAL' },
    faders: [
      fader('a', 'mic', 255),
      fader('b', 'chat', 128, true),
      fader('c', 'music', 0),
      fader('d', 'system', 51),
    ],
    channels: [],
    pressed: [],
    micLevelDb: -23.44,
    ...overrides,
  };
}

async function report(state: unknown) {
  feed.push(state);
  await tick();
}

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

  it('opens on the mixer and waits for the device', () => {
    render(App);
    expect(screen.getByRole('button', { name: 'Mixer' }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Mixer');
    expect(screen.getByText('Connecting…')).toBeTruthy();
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('still marks the sections that are not built as coming soon', async () => {
    render(App);
    await fireEvent.click(screen.getByRole('button', { name: 'Routing' }));
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

  describe('with the virtual device', () => {
    it('announces the demo mode', async () => {
      render(App);
      await report(snapshot());
      const banner = screen.getByRole('status');
      expect(banner.textContent).toContain('Demo mode');
      expect(banner.textContent).toContain('No GoXLR connected');
      expect(screen.queryByText('Connecting…')).toBeNull();
    });

    it('keeps the demo banner on every section', async () => {
      render(App);
      await report(snapshot());
      await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
      expect(screen.getByRole('status')).toBeTruthy();
    });

    it('draws the four faders with their channel and volume', async () => {
      render(App);
      await report(snapshot());
      const strips = screen.getAllByRole('article');
      expect(strips.map((strip) => strip.getAttribute('aria-label'))).toEqual([
        'Fader A',
        'Fader B',
        'Fader C',
        'Fader D',
      ]);
      expect(strips.map((strip) => within(strip).getByRole('strong').textContent)).toEqual([
        'Mic',
        'Chat',
        'Music',
        'System',
      ]);
      expect(strips.map((strip) => within(strip).getByText(/%$/).textContent)).toEqual([
        '100%',
        '50%',
        '0%',
        '20%',
      ]);
      expect(
        within(strips[3]).getByRole('meter', { name: 'System' }).getAttribute('aria-valuenow'),
      ).toBe('20');
    });

    it('says in words which channels are muted', async () => {
      render(App);
      await report(snapshot());
      const strips = screen.getAllByRole('article');
      expect(within(strips[0]).getByText('Live')).toBeTruthy();
      expect(within(strips[1]).getByText('Muted')).toBeTruthy();
    });

    it('follows the device when it changes', async () => {
      render(App);
      await report(snapshot());
      const state = snapshot({ micLevelDb: -6 });
      state.faders[2].volume = 255;
      await report(state);

      expect(within(screen.getAllByRole('article')[2]).getByText('100%')).toBeTruthy();
      expect(screen.getByText('-6.0 dB')).toBeTruthy();
    });

    it('shows the microphone level in decibels', async () => {
      render(App);
      await report(snapshot());
      const meter = screen.getByRole('meter', { name: 'Microphone level' });
      expect(meter.getAttribute('aria-valuetext')).toBe('-23.4 dB');
      expect(screen.getByText('-23.4 dB')).toBeTruthy();
    });

    it('survives a microphone level that is not a number', async () => {
      render(App);
      await report(snapshot({ micLevelDb: null as unknown as number }));
      expect(screen.getByText('-72.2 dB')).toBeTruthy();
      await report(snapshot({ micLevelDb: 12 }));
      expect(screen.getByText('0.0 dB')).toBeTruthy();
    });

    it('names the device', async () => {
      render(App);
      await report(snapshot());
      expect(screen.getByText('Virtual GoXLR')).toBeTruthy();
      expect(screen.getByText('1.4.3.110')).toBeTruthy();
    });

    it('speaks French too', async () => {
      render(App);
      await report(snapshot());
      i18n.setLocale('fr');
      await tick();

      expect(screen.getByRole('status').textContent).toContain('Mode démonstration');
      expect(screen.getByText('Coupé')).toBeTruthy();
      expect(screen.getByText('Musique')).toBeTruthy();
      expect(screen.getByText('GoXLR virtuelle')).toBeTruthy();
      expect(screen.getByText('-23,4 dB')).toBeTruthy();
    });
  });

  describe('with a real GoXLR', () => {
    const real = { kind: 'hardware', firmware: '1.4.3.110', serial: 'S1' } as const;

    it('shows no banner', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'hardware' }, device: real }));
      expect(screen.queryByRole('status')).toBeNull();
      expect(screen.getByText('GoXLR')).toBeTruthy();
    });

    it('goes back to the demo banner when it is unplugged', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'hardware' }, device: real }));
      await report(snapshot());
      expect(screen.getByRole('status').textContent).toContain('Demo mode');
    });
  });

  describe('when the GoXLR cannot be used', () => {
    it('names the program that must be closed', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'busy', program: 'GoXLR Utility' } }));
      const banner = screen.getByRole('status');
      expect(banner.textContent).toContain('GoXLR in use');
      expect(banner.textContent).toContain('Quit GoXLR Utility');
      expect(banner.textContent).not.toContain('{program}');
      expect(banner.textContent).not.toContain('Demo mode');

      i18n.setLocale('fr');
      await tick();
      expect(screen.getByRole('status').textContent).toContain('Quittez GoXLR Utility');
    });

    it('still asks to close the other program when its name is missing', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'busy' } }));
      const banner = screen.getByRole('status');
      expect(banner.textContent).toContain('Quit the other GoXLR program');
      expect(banner.textContent).not.toContain('undefined');
    });

    it('refuses a GoXLR Mini politely', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'unsupported' } }));
      expect(screen.getByRole('status').textContent).toContain('GoXLR Mini is not supported');

      i18n.setLocale('fr');
      await tick();
      expect(screen.getByRole('status').textContent).toContain('GoXLR Mini n’est pas prise en charge');
    });

    it('says when a GoXLR is plugged in but does not answer', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'unreachable' } }));
      expect(screen.getByRole('status').textContent).toContain('does not answer');

      i18n.setLocale('fr');
      await tick();
      expect(screen.getByRole('status').textContent).toContain('ne répond pas');
    });

    it('keeps showing the virtual device underneath', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'busy', program: 'GoXLR Utility' } }));
      expect(screen.getAllByRole('article')).toHaveLength(4);
      expect(screen.getByText('Virtual GoXLR')).toBeTruthy();
    });
  });

  describe('with a state it does not fully understand', () => {
    it('falls back on the kind of device when the connection is missing', async () => {
      render(App);
      const { connection: _, ...old } = snapshot();
      await report(old);
      expect(screen.getByRole('status').textContent).toContain('Demo mode');

      await report({ ...old, device: { kind: 'hardware', firmware: '1', serial: 'S1' } });
      expect(screen.queryByRole('status')).toBeNull();
    });

    it('treats an unknown connection state as the demo mode', async () => {
      render(App);
      await report(snapshot({ connection: { state: 'teleported' } as never }));
      expect(screen.getByRole('status').textContent).toContain('Demo mode');
    });
  });
});
