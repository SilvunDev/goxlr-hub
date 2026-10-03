import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import type { ChannelId, FaderId, Snapshot } from './lib/device';
import { i18n } from './lib/i18n/index.svelte';

// Stands in for the Rust side: `feed.push` plays the device reporting its state.
// `feed.sent` collects what the interface asks of the device.
const feed = vi.hoisted(() => ({
  push: (_snapshot: unknown) => {},
  sent: [] as unknown[],
}));

vi.mock('./lib/backend', () => ({
  syncLocale: async () => {},
  sendIntent: (intent: unknown) => {
    feed.sent.push(intent);
  },
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
    channels: [
      { channel: 'mic', volume: 255, muted: false, fader: 'a' },
      { channel: 'chat', volume: 128, muted: true, fader: 'b' },
      { channel: 'game', volume: 51, muted: false, fader: null },
      { channel: 'headphones', volume: null, muted: false, fader: null },
    ],
    micOff: false,
    routing: [
      { input: 'mic', outputs: ['broadcastMix', 'chatMic'] },
      { input: 'chat', outputs: ['headphones'] },
      { input: 'music', outputs: ['headphones', 'broadcastMix', 'lineOut'] },
      { input: 'samples', outputs: [] },
    ],
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
    feed.sent.length = 0;
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
    await fireEvent.click(screen.getByRole('button', { name: 'Controls' }));
    expect(screen.getByText('Coming soon')).toBeTruthy();
    await fireEvent.click(screen.getByRole('button', { name: 'Mixer' }));
    expect(screen.queryByText(/later version/)).toBeNull();
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
      expect(
        strips.map((strip) => (within(strip).getByRole('combobox') as HTMLSelectElement).value),
      ).toEqual(['mic', 'chat', 'music', 'system']);
      expect(strips.map((strip) => within(strip).getByText(/%$/).textContent)).toEqual([
        '100%',
        '50%',
        '0%',
        '20%',
      ]);
      expect(
        within(strips[3]).getByRole('slider', { name: 'System' }).getAttribute('aria-valuenow'),
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
      expect(screen.getByRole('slider', { name: 'Musique' })).toBeTruthy();
      expect(screen.getByText('GoXLR virtuelle')).toBeTruthy();
      expect(screen.getByText('-23,4 dB')).toBeTruthy();
    });
  });

  describe('when the mixer is handled from the screen', () => {
    it('sets the volume of a fader from the keyboard', async () => {
      render(App);
      await report(snapshot());
      const fader = screen.getByRole('slider', { name: 'Chat' });
      await fireEvent.keyDown(fader, { key: 'ArrowUp' });
      await fireEvent.keyDown(fader, { key: 'End' });
      await fireEvent.keyDown(fader, { key: 'Home' });
      expect(feed.sent).toEqual([
        { type: 'setVolume', channel: 'chat', volume: 131 },
        { type: 'setVolume', channel: 'chat', volume: 255 },
        { type: 'setVolume', channel: 'chat', volume: 0 },
      ]);
    });

    it('never asks for a volume out of range', async () => {
      render(App);
      await report(snapshot());
      await fireEvent.keyDown(screen.getByRole('slider', { name: 'Mic' }), { key: 'ArrowUp' });
      await fireEvent.keyDown(screen.getByRole('slider', { name: 'Music' }), { key: 'PageDown' });
      expect(feed.sent).toEqual([]);
    });

    it('sets the volume of a fader with the pointer', async () => {
      render(App);
      await report(snapshot());
      const fader = screen.getByRole('slider', { name: 'System' });
      fader.getBoundingClientRect = () => ({ top: 100, height: 200 }) as DOMRect;
      await fireEvent.pointerDown(fader, { clientY: 150, button: 0 });
      await fireEvent.pointerMove(fader, { clientY: 100 });
      await fireEvent.pointerMove(fader, { clientY: 20 });
      await fireEvent.pointerUp(fader);
      await fireEvent.pointerMove(fader, { clientY: 300 });
      expect(feed.sent).toEqual([
        { type: 'setVolume', channel: 'system', volume: 191 },
        { type: 'setVolume', channel: 'system', volume: 255 },
      ]);
      expect(within(screen.getAllByRole('article')[3]).getByText('100%')).toBeTruthy();
    });

    it('mutes a live channel and opens a muted one', async () => {
      render(App);
      await report(snapshot());
      const mic = screen.getByRole('button', { name: 'Mute Mic' });
      const chat = screen.getByRole('button', { name: 'Mute Chat' });
      expect(mic.getAttribute('aria-pressed')).toBe('false');
      expect(chat.getAttribute('aria-pressed')).toBe('true');
      await fireEvent.click(mic);
      await fireEvent.click(chat);
      expect(feed.sent).toEqual([
        { type: 'setMuted', channel: 'mic', muted: true },
        { type: 'setMuted', channel: 'chat', muted: false },
      ]);
    });

    it('turns the microphone off apart from the mute of its channel', async () => {
      render(App);
      await report(snapshot());
      const button = screen.getByRole('button', { name: 'Turn the microphone off' });
      expect(button.getAttribute('aria-pressed')).toBe('false');
      expect(button.textContent?.trim()).toBe('Live');
      await fireEvent.click(button);
      expect(feed.sent).toEqual([{ type: 'setMicOff', off: true }]);

      await report(snapshot({ micOff: true }));
      const off = screen.getByRole('button', { name: 'Turn the microphone off' });
      expect(off.getAttribute('aria-pressed')).toBe('true');
      expect(off.textContent?.trim()).toBe('Muted');
      expect(screen.getByRole('button', { name: 'Mute Mic' }).getAttribute('aria-pressed')).toBe(
        'false',
      );
      await fireEvent.click(off);
      expect(feed.sent[1]).toEqual({ type: 'setMicOff', off: false });
    });

    it('puts another channel under a fader', async () => {
      render(App);
      await report(snapshot());
      const source = screen.getByRole('combobox', { name: 'Channel of fader C' });
      expect(within(source).getAllByRole('option')).toHaveLength(11);
      await fireEvent.change(source, { target: { value: 'game' } });
      expect(feed.sent).toEqual([{ type: 'assignFader', fader: 'c', channel: 'game' }]);
    });

    it('keeps showing what the device says, not what was asked', async () => {
      render(App);
      await report(snapshot());
      await fireEvent.click(screen.getByRole('button', { name: 'Mute Mic' }));
      await report(snapshot());
      expect(screen.getByRole('button', { name: 'Mute Mic' }).getAttribute('aria-pressed')).toBe(
        'false',
      );
    });
  });

  describe('on the channels section', () => {
    async function open() {
      render(App);
      await report(snapshot());
      await fireEvent.click(screen.getByRole('button', { name: 'Channels' }));
    }

    it('lists every channel it is told about, with its fader', async () => {
      await open();
      const rows = screen.getAllByRole('listitem');
      expect(rows.map((row) => within(row).getByRole('slider').getAttribute('aria-label'))).toEqual(
        ['Mic', 'Chat', 'Game', 'Headphones'],
      );
      expect(within(rows[1]).getByText('Fader B')).toBeTruthy();
      expect(within(rows[1]).getByText('50%')).toBeTruthy();
      expect(within(rows[2]).queryByText(/Fader/)).toBeNull();
    });

    it('says in words when a volume is not known', async () => {
      await open();
      const row = screen.getAllByRole('listitem')[3];
      expect(within(row).getByText('Unknown')).toBeTruthy();
      expect(within(row).queryByText(/%/)).toBeNull();
      expect(screen.getByText(/cannot tell the volume/)).toBeTruthy();

      i18n.setLocale('fr');
      await tick();
      expect(within(screen.getAllByRole('listitem')[3]).getByText('Inconnu')).toBeTruthy();
    });

    it('does not talk about unknown volumes when all are known', async () => {
      render(App);
      const state = snapshot();
      state.channels = state.channels.slice(0, 3);
      await report(state);
      await fireEvent.click(screen.getByRole('button', { name: 'Channels' }));
      expect(screen.queryByText(/cannot tell the volume/)).toBeNull();
    });

    it('sets the volume and the mute of a channel that is on no fader', async () => {
      await open();
      await fireEvent.input(screen.getByRole('slider', { name: 'Headphones' }), {
        target: { value: '90' },
      });
      await fireEvent.click(screen.getByRole('button', { name: 'Mute Game' }));
      expect(feed.sent).toEqual([
        { type: 'setVolume', channel: 'headphones', volume: 90 },
        { type: 'setMuted', channel: 'game', muted: true },
      ]);
    });

    it('waits for the device', async () => {
      render(App);
      await fireEvent.click(screen.getByRole('button', { name: 'Channels' }));
      expect(screen.getByText('Connecting…')).toBeTruthy();
    });
  });

  describe('on the routing section', () => {
    async function open(state: unknown = snapshot()) {
      render(App);
      await report(state);
      await fireEvent.click(screen.getByRole('button', { name: 'Routing' }));
    }

    const cell = (name: string) => screen.getByRole('checkbox', { name });

    it('draws one row per source it is told about and one column per output', async () => {
      await open();
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Routing');
      expect(
        screen.getAllByRole('columnheader').map((header) => header.textContent?.trim()),
      ).toEqual([
        'Source',
        'Headphones',
        'Broadcast Mix',
        'Line Out',
        'Chat Mic',
        'Sampler',
      ]);
      expect(screen.getAllByRole('rowheader').map((header) => header.textContent?.trim())).toEqual([
        'Mic',
        'Chat',
        'Music',
        'Samples',
      ]);
      expect(screen.getAllByRole('checkbox')).toHaveLength(20);
    });

    it('ticks the outputs each source is sent to', async () => {
      await open();
      expect(cell('Mic to Broadcast Mix').getAttribute('aria-checked')).toBe('true');
      expect(cell('Mic to Chat Mic').getAttribute('aria-checked')).toBe('true');
      expect(cell('Mic to Headphones').getAttribute('aria-checked')).toBe('false');
      expect(cell('Music to Line Out').getAttribute('aria-checked')).toBe('true');
      expect(cell('Samples to Headphones').getAttribute('aria-checked')).toBe('false');
      expect(
        screen.getAllByRole('checkbox').filter((box) => box.getAttribute('aria-checked') === 'true'),
      ).toHaveLength(6);
    });

    it('asks to send a source to an output, or to stop', async () => {
      await open();
      await fireEvent.click(cell('Mic to Headphones'));
      await fireEvent.click(cell('Music to Broadcast Mix'));
      expect(feed.sent).toEqual([
        { type: 'setRoute', input: 'mic', output: 'headphones', on: true },
        { type: 'setRoute', input: 'music', output: 'broadcastMix', on: false },
      ]);
    });

    it('keeps showing what the device says, not what was clicked', async () => {
      await open();
      await fireEvent.click(cell('Mic to Headphones'));
      await report(snapshot());
      expect(cell('Mic to Headphones').getAttribute('aria-checked')).toBe('false');

      const state = snapshot();
      state.routing[0].outputs = ['headphones'];
      await report(state);
      expect(cell('Mic to Headphones').getAttribute('aria-checked')).toBe('true');
      expect(cell('Mic to Chat Mic').getAttribute('aria-checked')).toBe('false');
    });

    it('does not let a sound be sent back to where it comes from', async () => {
      await open();
      for (const name of ['Chat to Chat Mic', 'Samples to Sampler']) {
        const loop = cell(name) as HTMLButtonElement;
        expect(loop.disabled).toBe(true);
        expect(loop.getAttribute('aria-checked')).toBe('false');
        await fireEvent.click(loop);
      }
      expect(feed.sent).toEqual([]);
      expect((cell('Samples to Chat Mic') as HTMLButtonElement).disabled).toBe(false);
      expect(screen.getByText(/back to where it comes from/)).toBeTruthy();
    });

    it('never ticks a loop, even if the device says so', async () => {
      const state = snapshot();
      state.routing[1].outputs = ['chatMic'];
      await open(state);
      expect(cell('Chat to Chat Mic').getAttribute('aria-checked')).toBe('false');
    });

    it('says why the microphone may not be heard in the headphones', async () => {
      const state = (volume: number | null, outputs: string[]) => {
        const state = snapshot();
        state.routing[0].outputs = outputs as never;
        state.channels.push({ channel: 'micMonitor', volume, muted: false, fader: null });
        return state;
      };
      await open(state(null, ['headphones']));
      expect(screen.getByText(/Mic Monitor volume/)).toBeTruthy();
      await report(state(0, ['headphones']));
      expect(screen.getByText(/Mic Monitor volume/)).toBeTruthy();

      await report(state(200, ['headphones']));
      expect(screen.queryByText(/Mic Monitor volume/)).toBeNull();
      await report(state(null, ['chatMic']));
      expect(screen.queryByText(/Mic Monitor volume/)).toBeNull();
      // A device that does not list the channel: nothing to say.
      const bare = snapshot();
      bare.routing[0].outputs = ['headphones'];
      await report(bare);
      expect(screen.queryByText(/Mic Monitor volume/)).toBeNull();
    });

    it('waits for the device, and for a device that tells its routing', async () => {
      render(App);
      await fireEvent.click(screen.getByRole('button', { name: 'Routing' }));
      expect(screen.getByText('Connecting…')).toBeTruthy();

      const { routing: _, ...old } = snapshot();
      await report(old);
      expect(screen.getByText('Connecting…')).toBeTruthy();
      expect(screen.queryByRole('checkbox')).toBeNull();
    });

    it('speaks French too', async () => {
      await open();
      i18n.setLocale('fr');
      await tick();
      expect(cell('Musique vers Casque').getAttribute('aria-checked')).toBe('true');
      expect(screen.getByRole('columnheader', { name: /Mix de diffusion/ })).toBeTruthy();
      expect(screen.getByRole('columnheader', { name: /Micro du chat/ })).toBeTruthy();
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
