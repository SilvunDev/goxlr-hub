import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import type { ChannelId, FaderId, MicView, ProfilesView, Snapshot } from './lib/device';
import { i18n } from './lib/i18n/index.svelte';
import { updates } from './lib/updates.svelte';

// Stands in for the Rust side: `feed.push` plays the device reporting its state.
// `feed.sent` collects what the interface asks of the device.
// `feed.commands` collects what it asks of the profiles, answered by
// `feed.refusal`. `feed.askToQuit` plays the tray menu asking to quit.
const feed = vi.hoisted(() => ({
  push: (_snapshot: unknown) => {},
  sent: [] as unknown[],
  commands: [] as unknown[],
  refusal: null as string | null,
  startup: null as { enabled: boolean; hidden: boolean } | null,
  startupFails: false,
  askToQuit: () => {},
  quits: 0,
  updateSettings: null as { version: string; automatic: boolean; installs: boolean } | null,
  published: 'offline' as unknown,
  checks: 0,
  installs: [] as string[],
  installRefusal: null as string | null,
  opened: [] as string[],
}));

vi.mock('./lib/backend', () => ({
  syncLocale: async () => {},
  sendIntent: (intent: unknown) => {
    feed.sent.push(intent);
  },
  runProfileCommand: async (command: unknown) => {
    feed.commands.push(command);
    return feed.refusal;
  },
  getStartup: async () => feed.startup,
  setStartup: async (startup: { enabled: boolean; hidden: boolean }) => {
    if (feed.startupFails) return null;
    feed.startup = startup;
    return startup;
  },
  getUpdateSettings: async () => feed.updateSettings,
  setAutomaticUpdates: async (automatic: boolean) => {
    if (feed.updateSettings) feed.updateSettings = { ...feed.updateSettings, automatic };
    return feed.updateSettings;
  },
  getPublishedVersions: async () => {
    feed.checks += 1;
    return feed.published;
  },
  installVersion: async (version: string) => {
    feed.installs.push(version);
    return feed.installRefusal;
  },
  openRelease: async (version: string) => {
    feed.opened.push(version);
  },
  openBackups: async () => {
    feed.opened.push('backups');
  },
  onQuitRequested: (handler: () => void) => {
    feed.askToQuit = handler;
    return () => {};
  },
  quit: async () => {
    feed.quits += 1;
  },
  onDeviceState: (handler: (snapshot: unknown) => void) => {
    feed.push = handler;
    return () => {};
  },
}));

function mic(overrides: Partial<MicView> = {}): MicView {
  const band = (id: string, frequency: number, minFrequency: number, maxFrequency: number) => ({
    band: id as MicView['equalizer'][number]['band'],
    frequency,
    gain: 0,
    minFrequency,
    maxFrequency,
  });
  return {
    micType: 'dynamic',
    gain: 30,
    gate: { threshold: -30, attenuation: 100, attack: 0, release: 19 },
    compressor: { threshold: 0, ratio: 9, attack: 1, release: 9, makeupGain: 0 },
    equalizer: [
      band('hz31', 31.5, 30, 63),
      band('hz63', 63, 31.5, 125),
      band('hz125', 125, 63, 250),
      band('hz250', 250, 125, 300),
      band('hz500', 500, 300, 1000),
      band('khz1', 1000, 500, 2000),
      band('khz2', 2000, 1000, 2000),
      band('khz4', 4000, 2000, 8000),
      band('khz8', 8000, 4000, 16000),
      band('khz16', 16000, 8000, 18000),
    ],
    deEsser: 0,
    ...overrides,
  };
}

function profiles(overrides: Partial<ProfilesView> = {}): ProfilesView {
  return {
    active: { profile: 'Stream', mix: 'Desk', mic: 'Radio', controls: 'Keys' },
    profiles: ['Game', 'Stream'],
    mixes: ['Desk', 'Quiet'],
    mics: ['Headset', 'Radio'],
    controls: ['Keys', 'Pads'],
    dirty: { profile: false, mix: false, mic: false, controls: false },
    unsaved: false,
    ...overrides,
  };
}

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
    mic: mic(),
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
    feed.commands.length = 0;
    feed.refusal = null;
    feed.startup = null;
    feed.startupFails = false;
    feed.quits = 0;
    feed.updateSettings = null;
    feed.published = 'offline';
    feed.checks = 0;
    feed.installs.length = 0;
    feed.installRefusal = null;
    feed.opened.length = 0;
    updates.reset();
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
    await fireEvent.click(screen.getByRole('button', { name: 'Lighting' }));
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

    it('lights the sampler pads that are held on the device, and opens their case on a click', async () => {
      render(App);
      await report(snapshot());
      const pads = within(screen.getByRole('region', { name: 'Sampler pads' }));
      expect(pads.getAllByRole('listitem').map((pad) => pad.getAttribute('aria-label'))).toEqual([
        'Bank A, released',
        'Bank B, released',
        'Bank C, released',
        'Top left, released',
        'Top right, released',
        'Bottom left, released',
        'Bottom right, released',
        'Clear, released',
      ]);
      // Each one is a way into its case in Controls, and nothing else.
      expect(pads.getAllByRole('button')).toHaveLength(8);

      await report(snapshot({ pressed: ['samplerTopRight', 'samplerSelectB', 'fader1Mute'] }));
      expect(pads.getByLabelText('Top right, pressed')).toBeTruthy();
      expect(pads.getByLabelText('Bank B, pressed')).toBeTruthy();
      expect(pads.getByLabelText('Top left, released')).toBeTruthy();

      // A device that tells no button shows no pad held, and does not break.
      await report({ ...snapshot(), pressed: undefined });
      expect(pads.getByLabelText('Top right, released')).toBeTruthy();
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

  describe('on the microphone section', () => {
    async function open(state: unknown = snapshot()) {
      render(App);
      await report(state);
      await fireEvent.click(screen.getByRole('button', { name: 'Microphone' }));
    }
    const slider = (name: string) => screen.getByRole('slider', { name }) as HTMLInputElement;
    const radio = (name: string) => screen.getByRole('radio', { name });
    async function set(name: string, value: number) {
      await fireEvent.input(slider(name), { target: { value: String(value) } });
    }

    it('shows the level meter and every part of the processing', async () => {
      await open();
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Microphone');
      expect(screen.getByRole('meter').getAttribute('aria-valuetext')).toBe('-23.4 dB');
      expect(
        screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent),
      ).toEqual(['Input', 'Noise gate', 'Compressor', 'Equaliser', 'De-esser']);
      expect(screen.getAllByRole('radio')).toHaveLength(3);
      expect(screen.queryByText('Coming soon')).toBeNull();
    });

    it('says the type and the gain of the microphone', async () => {
      await open();
      expect(radio('Dynamic').getAttribute('aria-checked')).toBe('true');
      expect(radio('Condenser').getAttribute('aria-checked')).toBe('false');
      expect(slider('Gain').value).toBe('30');
      expect(slider('Gain').getAttribute('aria-valuetext')).toBe('30 dB');
      expect(screen.queryByText(/48 V/)).toBeNull();
      expect(screen.queryByText(/Choose your microphone type/)).toBeNull();
    });

    it('asks for the microphone type before anything is sent to it', async () => {
      await open(snapshot({ mic: mic({ micType: null, gain: null }) }));
      for (const name of ['Dynamic', 'Condenser', '3.5 mm jack']) {
        expect(radio(name).getAttribute('aria-checked')).toBe('false');
      }
      expect(screen.getByText(/Choose your microphone type/)).toBeTruthy();
      expect(slider('Gain').disabled).toBe(true);
      await set('Gain', 40);
      expect(feed.sent).toEqual([]);

      await fireEvent.click(radio('3.5 mm jack'));
      expect(feed.sent).toEqual([{ type: 'setMicType', micType: 'jack' }]);
      // The screen waits for the device to say so.
      expect(radio('3.5 mm jack').getAttribute('aria-checked')).toBe('false');
    });

    it('warns that a condenser gets phantom power', async () => {
      await open(snapshot({ mic: mic({ micType: 'condenser', gain: 12 }) }));
      expect(radio('Condenser').getAttribute('aria-checked')).toBe('true');
      expect(screen.getByText(/48 V phantom power/)).toBeTruthy();
    });

    it('sets the gain within what the preamp gives', async () => {
      await open();
      expect(slider('Gain').max).toBe('72');
      await set('Gain', 45);
      // The slider itself stops at its ends.
      await set('Gain', 73);
      expect(feed.sent).toEqual([
        { type: 'setMicGain', gain: 45 },
        { type: 'setMicGain', gain: 72 },
      ]);
    });

    it('sets the gate, and tells times in milliseconds', async () => {
      await open();
      expect(slider('Noise gate: Threshold').getAttribute('aria-valuetext')).toBe('-30 dB');
      expect(slider('Noise gate: Attenuation').getAttribute('aria-valuetext')).toBe('100%');
      expect(slider('Noise gate: Attack').getAttribute('aria-valuetext')).toBe('10 ms');
      expect(slider('Noise gate: Release').getAttribute('aria-valuetext')).toBe('200 ms');

      await set('Noise gate: Threshold', -45);
      await set('Noise gate: Attenuation', 60);
      await set('Noise gate: Attack', 3);
      await set('Noise gate: Release', 45);
      expect(feed.sent).toEqual([
        { type: 'setGate', setting: 'threshold', value: -45 },
        { type: 'setGate', setting: 'attenuation', value: 60 },
        { type: 'setGate', setting: 'attack', value: 3 },
        { type: 'setGate', setting: 'release', value: 45 },
      ]);
      // Shown at once, before the device confirms.
      expect(slider('Noise gate: Release').getAttribute('aria-valuetext')).toBe('2000 ms');
    });

    it('sets the compressor, and tells the ratio as a ratio', async () => {
      await open();
      expect(slider('Compressor: Ratio').getAttribute('aria-valuetext')).toBe('4:1');
      expect(slider('Compressor: Attack').getAttribute('aria-valuetext')).toBe('2 ms');
      expect(slider('Compressor: Release').getAttribute('aria-valuetext')).toBe('100 ms');
      expect(slider('Compressor: Make-up gain').getAttribute('aria-valuetext')).toBe('0 dB');

      await set('Compressor: Threshold', -20);
      await set('Compressor: Ratio', 7);
      await set('Compressor: Attack', 19);
      await set('Compressor: Release', 0);
      await set('Compressor: Make-up gain', 6);
      expect(feed.sent).toEqual([
        { type: 'setCompressor', setting: 'threshold', value: -20 },
        { type: 'setCompressor', setting: 'ratio', value: 7 },
        { type: 'setCompressor', setting: 'attack', value: 19 },
        { type: 'setCompressor', setting: 'release', value: 0 },
        { type: 'setCompressor', setting: 'makeupGain', value: 6 },
      ]);
      expect(slider('Compressor: Ratio').getAttribute('aria-valuetext')).toBe('2.5:1');
    });

    it('puts each part of the processing back to neutral, or all of them', async () => {
      await open();
      for (const group of ['Noise gate', 'Compressor', 'Equaliser', 'De-esser']) {
        await fireEvent.click(screen.getByRole('button', { name: `Reset to neutral: ${group}` }));
      }
      await fireEvent.click(
        screen.getByRole('button', { name: 'Reset all the processing to neutral' }),
      );
      expect(feed.sent).toEqual(
        ['gate', 'compressor', 'equalizer', 'deEsser', 'all'].map((block) => ({
          type: 'resetMic',
          block,
        })),
      );
      // The type and the gain are no processing: nothing resets them.
      expect(screen.queryByRole('button', { name: 'Reset to neutral: Input' })).toBeNull();
    });

    it('sets the de-esser', async () => {
      await open(snapshot({ mic: mic({ deEsser: 20 }) }));
      expect(slider('De-esser: Amount').value).toBe('20');
      await set('De-esser: Amount', 55);
      expect(feed.sent).toEqual([{ type: 'setDeEsser', amount: 55 }]);
    });

    it('draws one point per band of the equaliser', async () => {
      const state = mic();
      state.equalizer[5].gain = 4;
      await open(snapshot({ mic: state }));
      const points = screen.getAllByRole('slider', { name: /Equaliser band/ });
      expect(points).toHaveLength(10);
      expect(points[0].getAttribute('aria-valuetext')).toBe('31.5 Hz, 0 dB');
      expect(points[5].getAttribute('aria-valuetext')).toBe('1000 Hz, +4 dB');
      expect(points[5].getAttribute('aria-valuenow')).toBe('4');
      expect(points[9].getAttribute('aria-valuetext')).toBe('16000 Hz, 0 dB');
    });

    it('moves a band of the equaliser with the arrow keys', async () => {
      await open();
      const point = screen.getByRole('slider', { name: 'Equaliser band 6' });
      await fireEvent.keyDown(point, { key: 'ArrowUp' });
      expect(feed.sent).toEqual([{ type: 'setEqBand', band: 'khz1', frequency: 1000, gain: 1 }]);
      // Shown at once: the next key starts from there.
      expect(point.getAttribute('aria-valuetext')).toBe('1000 Hz, +1 dB');
      await fireEvent.keyDown(point, { key: 'ArrowRight' });
      await fireEvent.keyDown(point, { key: 'ArrowDown' });
      await fireEvent.keyDown(point, { key: 'ArrowDown' });
      await fireEvent.keyDown(point, { key: 'ArrowLeft' });
      expect(feed.sent.slice(1)).toEqual([
        { type: 'setEqBand', band: 'khz1', frequency: 1030, gain: 1 },
        { type: 'setEqBand', band: 'khz1', frequency: 1030, gain: 0 },
        { type: 'setEqBand', band: 'khz1', frequency: 1030, gain: -1 },
        { type: 'setEqBand', band: 'khz1', frequency: 1000, gain: -1 },
      ]);
      await fireEvent.keyDown(point, { key: 'a' });
      expect(feed.sent).toHaveLength(5);
    });

    it('keeps a band within its gain and between its neighbours', async () => {
      const state = mic();
      state.equalizer[6] = { ...state.equalizer[6], gain: 9 };
      state.equalizer[0] = { ...state.equalizer[0], frequency: 30, gain: -9 };
      await open(snapshot({ mic: state }));

      // 2 kHz already sits at the end of its range, and at full gain.
      const top = screen.getByRole('slider', { name: 'Equaliser band 7' });
      await fireEvent.keyDown(top, { key: 'ArrowUp' });
      await fireEvent.keyDown(top, { key: 'ArrowRight' });
      const bottom = screen.getByRole('slider', { name: 'Equaliser band 1' });
      await fireEvent.keyDown(bottom, { key: 'ArrowDown' });
      await fireEvent.keyDown(bottom, { key: 'ArrowLeft' });
      expect(feed.sent).toEqual([]);
    });

    it('follows the device when it changes', async () => {
      await open();
      const state = mic();
      state.gate.threshold = -12;
      state.equalizer[2] = { ...state.equalizer[2], frequency: 180, gain: -3 };
      await report(snapshot({ mic: state }));
      expect(slider('Noise gate: Threshold').value).toBe('-12');
      expect(
        screen.getByRole('slider', { name: 'Equaliser band 3' }).getAttribute('aria-valuetext'),
      ).toBe('180 Hz, -3 dB');
    });

    it('waits for the device, and for a device that tells its microphone', async () => {
      render(App);
      await fireEvent.click(screen.getByRole('button', { name: 'Microphone' }));
      expect(screen.getByText('Connecting…')).toBeTruthy();

      const { mic: _, ...old } = snapshot();
      await report(old);
      expect(screen.getByText('Connecting…')).toBeTruthy();
      expect(screen.queryByRole('slider')).toBeNull();
    });

    it('speaks French too', async () => {
      await open(snapshot({ mic: mic({ micType: 'condenser', gain: 12 }) }));
      i18n.setLocale('fr');
      await tick();
      expect(radio('Condensateur').getAttribute('aria-checked')).toBe('true');
      expect(screen.getByText(/alimentation fantôme 48 V/)).toBeTruthy();
      expect(slider('Compresseur : Gain de rattrapage')).toBeTruthy();
      expect(slider('Compresseur : Ratio').getAttribute('aria-valuetext')).toBe('4:1');
      expect(screen.getByRole('slider', { name: 'Bande 1 de l’égaliseur' })).toBeTruthy();
      expect(
        screen.getByRole('slider', { name: 'Bande 1 de l’égaliseur' }).getAttribute('aria-valuetext'),
      ).toBe('31,5 Hz, 0 dB');
    });
  });

  describe('with profiles', () => {
    const unsaved = () =>
      profiles({ dirty: { profile: false, mix: false, mic: true, controls: false }, unsaved: true });
    const card = (name: string) => within(screen.getByRole('region', { name }));
    const banner = () => screen.queryByRole('region', { name: 'Unsaved changes' });

    async function open(view: ProfilesView = profiles()) {
      render(App);
      await report(snapshot({ profiles: view }));
      await fireEvent.click(screen.getByRole('button', { name: 'Manage profiles' }));
    }
    async function type(name: string) {
      await fireEvent.input(screen.getByRole('textbox', { name: 'Name' }), {
        target: { value: name },
      });
    }

    it('names the profile in use at the top of every section', async () => {
      render(App);
      expect(screen.getByRole('banner').textContent).toContain('No profile yet');
      await report(snapshot({ profiles: profiles() }));
      expect(screen.getByRole('banner').textContent).toContain('Stream');
      await fireEvent.click(screen.getByRole('button', { name: 'Routing' }));
      expect(screen.getByRole('banner').textContent).toContain('Stream');
    });

    it('says nothing of unsaved changes when there are none', async () => {
      render(App);
      await report(snapshot({ profiles: profiles() }));
      expect(banner()).toBeNull();
    });

    it('shows a banner while something is not saved, and saves from it', async () => {
      render(App);
      await report(snapshot({ profiles: unsaved() }));
      expect(banner()?.textContent).toContain('Unsaved changes');
      await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
      expect(banner()).toBeTruthy();

      await fireEvent.click(within(banner()!).getByRole('button', { name: 'Save' }));
      expect(feed.commands).toEqual([{ type: 'save', kind: 'profile' }]);
      // The banner goes when the device says so, not when the button is clicked.
      expect(banner()).toBeTruthy();
      await report(snapshot({ profiles: profiles() }));
      expect(banner()).toBeNull();
    });

    it('says why saving from the banner did not work', async () => {
      feed.refusal = 'storage';
      render(App);
      await report(snapshot({ profiles: unsaved() }));
      await fireEvent.click(within(banner()!).getByRole('button', { name: 'Save' }));
      await tick();
      expect(screen.getByRole('alert').textContent).toBe('Could not write to the disk.');
    });

    it('lists the profiles and the pieces, with what is in use', async () => {
      await open();
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Profiles');
      expect(
        screen.getAllByRole('heading', { level: 2 }).map((heading) => heading.textContent),
      ).toEqual(['Profiles', 'Mixes', 'Microphones', 'Controls']);

      const names = (region: string) =>
        card(region)
          .getAllByRole('radio')
          .map((radio) => [radio.textContent?.trim(), radio.getAttribute('aria-checked')]);
      expect(names('Profiles')).toEqual([
        ['Game', 'false'],
        ['Stream In use', 'true'],
      ]);
      expect(names('Mixes')).toEqual([
        ['Desk In use', 'true'],
        ['Quiet', 'false'],
      ]);
      expect(names('Microphones')).toEqual([
        ['Headset', 'false'],
        ['Radio In use', 'true'],
      ]);
      expect(names('Controls')).toEqual([
        ['Keys In use', 'true'],
        ['Pads', 'false'],
      ]);
      expect(screen.getByText(/Lighting will become a piece/)).toBeTruthy();
    });

    it('switches to another profile or to another piece', async () => {
      await open();
      await fireEvent.click(card('Profiles').getByRole('radio', { name: /Game/ }));
      await fireEvent.click(card('Microphones').getByRole('radio', { name: /Headset/ }));
      // The one in use is not loaded again.
      await fireEvent.click(card('Mixes').getByRole('radio', { name: /Desk/ }));
      expect(feed.commands).toEqual([
        { type: 'select', kind: 'profile', name: 'Game' },
        { type: 'select', kind: 'mic', name: 'Headset' },
      ]);
    });

    it('asks before a switch that would lose unsaved changes', async () => {
      await open(unsaved());
      expect(card('Microphones').getByRole('radio', { name: /Radio/ }).textContent).toContain(
        'Not saved',
      );

      // Another mix leaves the microphone alone: nothing to lose.
      await fireEvent.click(card('Mixes').getByRole('radio', { name: /Quiet/ }));
      expect(feed.commands).toEqual([{ type: 'select', kind: 'mix', name: 'Quiet' }]);

      await fireEvent.click(card('Microphones').getByRole('radio', { name: /Headset/ }));
      expect(feed.commands).toHaveLength(1);
      expect(screen.getByText(/Switch to “Headset”\?/)).toBeTruthy();
      await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
      expect(screen.queryByText(/Switch to/)).toBeNull();
      expect(feed.commands).toHaveLength(1);

      await fireEvent.click(card('Profiles').getByRole('radio', { name: /Game/ }));
      await fireEvent.click(screen.getByRole('button', { name: 'Switch anyway' }));
      expect(feed.commands[1]).toEqual({ type: 'select', kind: 'profile', name: 'Game' });
    });

    it('saves a piece alone, only when it changed', async () => {
      await open(unsaved());
      const save = (region: string) =>
        card(region).getByRole('button', { name: 'Save' }) as HTMLButtonElement;
      expect(save('Mixes').disabled).toBe(true);
      expect(save('Microphones').disabled).toBe(false);
      // A profile is saved with its pieces.
      expect(save('Profiles').disabled).toBe(false);

      await fireEvent.click(save('Microphones'));
      expect(feed.commands).toEqual([{ type: 'save', kind: 'mic' }]);
    });

    it('saves under a new name typed on the screen', async () => {
      await open();
      await fireEvent.click(card('Profiles').getByRole('button', { name: 'Save as…' }));
      await type('Late night');
      await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
      expect(feed.commands).toEqual([{ type: 'saveAs', kind: 'profile', name: 'Late night' }]);
      await tick();
      expect(screen.queryByRole('textbox')).toBeNull();
    });

    it('renames and duplicates, starting from the name it has', async () => {
      await open();
      await fireEvent.click(screen.getByRole('button', { name: 'Rename Quiet' }));
      const field = screen.getByRole('textbox', { name: 'Name' }) as HTMLInputElement;
      expect(field.value).toBe('Quiet');
      await type('Night');
      await fireEvent.submit(field.form!);

      await tick();
      await fireEvent.click(screen.getByRole('button', { name: 'Duplicate Radio' }));
      await type('Radio 2');
      await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
      expect(feed.commands).toEqual([
        { type: 'rename', kind: 'mix', name: 'Quiet', to: 'Night' },
        { type: 'duplicate', kind: 'mic', name: 'Radio', to: 'Radio 2' },
      ]);
    });

    it('deletes only after a yes, and never what is in use', async () => {
      await open();
      const remove = (name: string) =>
        screen.getByRole('button', { name: `Delete ${name}` }) as HTMLButtonElement;
      expect(remove('Stream').disabled).toBe(true);
      expect(remove('Desk').disabled).toBe(true);

      await fireEvent.click(remove('Game'));
      expect(screen.getByText('Delete “Game”? This cannot be undone.')).toBeTruthy();
      expect(feed.commands).toEqual([]);
      await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
      expect(feed.commands).toEqual([]);

      await fireEvent.click(remove('Game'));
      await fireEvent.click(card('Profiles').getByRole('button', { name: 'Delete' }));
      expect(feed.commands).toEqual([{ type: 'delete', kind: 'profile', name: 'Game' }]);
    });

    it('says in plain words why something was refused, and keeps the name typed', async () => {
      await open();
      for (const [refusal, said] of [
        ['nameTaken', 'This name is already taken.'],
        ['invalidName', /This name cannot be used/],
        ['inUse', /It is in use/],
        ['unreadable', 'Its file cannot be read.'],
      ] as const) {
        feed.refusal = refusal;
        await fireEvent.click(card('Mixes').getByRole('button', { name: 'Save as…' }));
        await type('Desk');
        await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
        await tick();
        expect(card('Mixes').getByRole('alert').textContent).toMatch(said);
        expect((screen.getByRole('textbox', { name: 'Name' }) as HTMLInputElement).value).toBe(
          'Desk',
        );
        await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
        expect(screen.queryByRole('alert')).toBeNull();
      }
    });

    it('waits for a device that tells its profiles', async () => {
      render(App);
      await fireEvent.click(screen.getByRole('button', { name: 'Manage profiles' }));
      expect(screen.getByText('Connecting…')).toBeTruthy();
      await report(snapshot());
      expect(screen.getByText('Connecting…')).toBeTruthy();
      await report(snapshot({ profiles: { active: {} } as unknown as ProfilesView }));
      expect(screen.getByText('Connecting…')).toBeTruthy();
    });

    it('speaks French too', async () => {
      await open(unsaved());
      i18n.setLocale('fr');
      await tick();
      expect(screen.getByRole('region', { name: 'Modifications non enregistrées' })).toBeTruthy();
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Profils');
      expect(card('Micros').getByRole('radio', { name: /Radio/ }).textContent).toContain(
        'Non enregistré',
      );
      expect(screen.getByRole('button', { name: 'Renommer Quiet' })).toBeTruthy();
    });
  });

  describe('when asked to quit with unsaved changes', () => {
    async function ask() {
      render(App);
      await report(
        snapshot({
          profiles: profiles({ dirty: { profile: false, mix: true, mic: false, controls: false }, unsaved: true }),
        }),
      );
      feed.askToQuit();
      await tick();
      return within(screen.getByRole('alertdialog', { name: 'Quit without saving?' }));
    }

    it('asks nothing until then', async () => {
      render(App);
      await report(snapshot({ profiles: profiles() }));
      expect(screen.queryByRole('alertdialog')).toBeNull();
    });

    it('saves and quits', async () => {
      const dialog = await ask();
      await fireEvent.click(dialog.getByRole('button', { name: 'Save and quit' }));
      await tick();
      expect(feed.commands).toEqual([{ type: 'save', kind: 'profile' }]);
      expect(feed.quits).toBe(1);
    });

    it('stays open and says why when saving did not work', async () => {
      feed.refusal = 'storage';
      const dialog = await ask();
      await fireEvent.click(dialog.getByRole('button', { name: 'Save and quit' }));
      await tick();
      expect(feed.quits).toBe(0);
      expect(dialog.getByRole('alert').textContent).toBe('Could not write to the disk.');
    });

    it('quits without saving', async () => {
      const dialog = await ask();
      await fireEvent.click(dialog.getByRole('button', { name: 'Quit without saving' }));
      expect(feed.commands).toEqual([]);
      expect(feed.quits).toBe(1);
    });

    it('goes back to the app on cancel or Escape', async () => {
      const dialog = await ask();
      await fireEvent.click(dialog.getByRole('button', { name: 'Cancel' }));
      expect(screen.queryByRole('alertdialog')).toBeNull();

      feed.askToQuit();
      await tick();
      await fireEvent.keyDown(window, { key: 'Escape' });
      expect(screen.queryByRole('alertdialog')).toBeNull();
      expect(feed.quits).toBe(0);
    });
  });

  describe('with published versions', () => {
    const published = (version: string, flags: Partial<Record<string, boolean>> = {}) => ({
      version,
      published: '2026-10-04T10:00:00Z',
      current: false,
      newer: false,
      installable: true,
      ...flags,
    });
    const three = [
      published('0.3.0', { newer: true }),
      published('0.2.0', { current: true }),
      published('0.1.0'),
    ];
    const windows = { version: '0.2.0', automatic: true, installs: true };
    const banner = () => screen.queryByRole('region', { name: 'New version' });

    /** Lets the settings be read, then the versions. */
    async function settle() {
      for (let turn = 0; turn < 4; turn += 1) await tick();
    }

    async function start() {
      render(App);
      await settle();
    }

    async function openSettings() {
      await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
      await settle();
    }

    it('tells about a newer version and installs nothing', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      await start();
      expect(banner()?.textContent).toContain('GoXLR Hub 0.3.0 is out');
      expect(feed.installs).toEqual([]);

      await fireEvent.click(within(banner()!).getByRole('button', { name: 'See' }));
      await settle();
      expect(screen.getByRole('heading', { name: 'Settings' })).toBeTruthy();
      expect(banner()).toBeNull();
    });

    it('stops telling about a version the user put off', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      await start();
      await fireEvent.click(within(banner()!).getByRole('button', { name: 'Later' }));
      await settle();
      expect(banner()).toBeNull();
    });

    it('says nothing when the version running is the latest', async () => {
      feed.updateSettings = { ...windows, version: '0.3.0' };
      feed.published = [published('0.3.0', { current: true }), published('0.2.0')];
      await start();
      expect(banner()).toBeNull();
      await openSettings();
      expect(screen.getByText('It is the latest version.')).toBeTruthy();
    });

    it('does not ask GitHub by itself when the user said not to', async () => {
      feed.updateSettings = { ...windows, automatic: false };
      feed.published = three;
      await start();
      expect(feed.checks).toBe(0);
      expect(banner()).toBeNull();

      await openSettings();
      await fireEvent.click(screen.getByRole('button', { name: 'Look for versions now' }));
      await settle();
      expect(feed.checks).toBe(1);
      expect(within(screen.getByRole('list', { name: 'Published versions' })).getAllByRole('listitem')).toHaveLength(3);
    });

    it('keeps a failed check to itself unless the user asked', async () => {
      feed.updateSettings = windows;
      await start();
      await openSettings();
      expect(screen.queryByRole('alert')).toBeNull();

      await fireEvent.click(screen.getByRole('button', { name: 'Look for versions now' }));
      await settle();
      expect(screen.getByRole('alert').textContent).toContain('GitHub could not be reached');
    });

    it('turns the automatic check off and on', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      await start();
      await openSettings();
      const box = () => screen.getByRole('checkbox', { name: /Look for a new version/ }) as HTMLInputElement;
      expect(box().checked).toBe(true);
      await fireEvent.click(box());
      await settle();
      expect(feed.updateSettings?.automatic).toBe(false);
      expect(box().checked).toBe(false);
    });

    it('installs the version the user chose, newer or older, once confirmed', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      await start();
      await openSettings();
      // The version running cannot be installed over itself.
      expect(screen.queryByRole('button', { name: /0\.2\.0/ })).toBeNull();

      await fireEvent.click(screen.getByRole('button', { name: 'Install 0.3.0' }));
      await settle();
      expect(feed.installs).toEqual([]);
      await fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
      await settle();
      expect(feed.installs).toEqual(['0.3.0']);

      await fireEvent.click(screen.getByRole('button', { name: 'Go back to 0.1.0' }));
      await settle();
      expect(screen.getByText(/an older version cannot load them/)).toBeTruthy();
      await fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
      await settle();
      expect(feed.installs).toEqual(['0.3.0', '0.1.0']);
    });

    it('asks again before nothing: cancelling installs nothing', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      await start();
      await openSettings();
      await fireEvent.click(screen.getByRole('button', { name: 'Install 0.3.0' }));
      await settle();
      await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
      await settle();
      expect(screen.queryByRole('button', { name: 'Install and restart' })).toBeNull();
      expect(feed.installs).toEqual([]);
    });

    it('says why a version was not installed', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      feed.installRefusal = 'corrupt';
      await start();
      await openSettings();
      await fireEvent.click(screen.getByRole('button', { name: 'Install 0.3.0' }));
      await settle();
      await fireEvent.click(screen.getByRole('button', { name: 'Install and restart' }));
      await settle();
      expect(screen.getByRole('alert').textContent).toContain('Nothing was installed');
    });

    it('installs nothing while something is not saved', async () => {
      feed.updateSettings = windows;
      feed.published = three;
      await start();
      feed.push(snapshot({ profiles: profiles({ unsaved: true }) }));
      await openSettings();
      await fireEvent.click(screen.getByRole('button', { name: 'Install 0.3.0' }));
      await settle();
      const confirm = screen.getByRole('button', { name: 'Install and restart' });
      expect((confirm as HTMLButtonElement).disabled).toBe(true);
      expect(screen.getByText(/Save your changes first/)).toBeTruthy();
    });

    it('offers no installer for a version that has none', async () => {
      feed.updateSettings = windows;
      feed.published = [published('0.3.0', { newer: true, installable: false }), three[1]];
      await start();
      await openSettings();
      expect(screen.queryByRole('button', { name: 'Install 0.3.0' })).toBeNull();
      await fireEvent.click(screen.getAllByRole('button', { name: 'What changed' })[0]);
      expect(feed.opened).toEqual(['0.3.0']);
    });

    it('sends to the download page where the package manager installs', async () => {
      feed.updateSettings = { ...windows, installs: false };
      feed.published = three.map((version) => ({ ...version, installable: false }));
      await start();
      expect(banner()).not.toBeNull();
      await openSettings();
      expect(screen.queryByRole('button', { name: /^Install/ })).toBeNull();
      await fireEvent.click(screen.getAllByRole('button', { name: 'Download page' })[2]);
      expect(feed.opened).toEqual(['0.1.0']);
      expect(screen.getByText(/package manager/)).toBeTruthy();
    });

    it('opens the profile backups', async () => {
      feed.updateSettings = windows;
      await start();
      await openSettings();
      await fireEvent.click(screen.getByRole('button', { name: 'Open the profile backups' }));
      expect(feed.opened).toEqual(['backups']);
    });
  });

  describe('on the startup setting', () => {
    const box = (name: RegExp) => screen.getByRole('checkbox', { name }) as HTMLInputElement;
    async function open() {
      render(App);
      await fireEvent.click(screen.getByRole('button', { name: 'Settings' }));
      await tick();
    }

    it('is not shown when the system cannot tell', async () => {
      await open();
      expect(screen.queryByRole('checkbox')).toBeNull();
    });

    it('shows how the app starts, hidden by default', async () => {
      feed.startup = { enabled: false, hidden: true };
      await open();
      expect(box(/with the computer/).checked).toBe(false);
      expect(box(/stay hidden/).checked).toBe(true);
      expect(box(/stay hidden/).disabled).toBe(true);
    });

    it('starts the app with the computer, hidden or not', async () => {
      feed.startup = { enabled: false, hidden: true };
      await open();
      await fireEvent.click(box(/with the computer/));
      await tick();
      expect(feed.startup).toEqual({ enabled: true, hidden: true });
      expect(box(/with the computer/).checked).toBe(true);
      expect(box(/stay hidden/).disabled).toBe(false);

      await fireEvent.click(box(/stay hidden/));
      await tick();
      expect(feed.startup).toEqual({ enabled: true, hidden: false });
      expect(box(/stay hidden/).checked).toBe(false);
    });

    it('keeps showing what the system says when the change fails', async () => {
      feed.startup = { enabled: false, hidden: true };
      feed.startupFails = true;
      await open();
      await fireEvent.click(box(/with the computer/));
      await tick();
      expect(box(/with the computer/).checked).toBe(false);
      expect(screen.getByRole('alert').textContent).toBe('This setting could not be changed.');
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
