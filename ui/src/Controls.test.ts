import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App.svelte';
import {
  BUTTONS,
  type Action,
  type ButtonId,
  type ButtonView,
  type ControlsView,
  type MicView,
  type Snapshot,
} from './lib/device';
import { i18n } from './lib/i18n/index.svelte';
import { updates } from './lib/updates.svelte';

// Stands in for the Rust side: `feed.push` plays the device reporting its state,
// `feed.sent` collects what the interface asks of it.
const feed = vi.hoisted(() => ({
  push: (_snapshot: unknown) => {},
  sent: [] as unknown[],
  commands: [] as unknown[],
}));

vi.mock('./lib/backend', () => ({
  syncLocale: async () => {},
  sendIntent: (intent: unknown) => {
    feed.sent.push(intent);
  },
  runProfileCommand: async (command: unknown) => {
    feed.commands.push(command);
    return null;
  },
  getStartup: async () => null,
  setStartup: async () => null,
  getUpdateSettings: async () => null,
  setAutomaticUpdates: async () => null,
  getPublishedVersions: async () => 'offline',
  installVersion: async () => null,
  openRelease: async () => {},
  openBackups: async () => {},
  onQuitRequested: () => () => {},
  quit: async () => {},
  onDeviceState: (handler: (snapshot: unknown) => void) => {
    feed.push = handler;
    return () => {};
  },
}));

const mute = (
  target: Extract<Action, { type: 'mute' }>['target'],
  mode: 'mute' | 'unmute' | 'toggle' = 'toggle',
): Action => ({ type: 'mute', target, mode });

const under = (fader: 'a' | 'b' | 'c' | 'd') => mute({ type: 'faderTrack', fader });
const track = (channel: 'music' | 'chat', mode: 'mute' | 'unmute' | 'toggle' = 'mute') =>
  mute({ type: 'channel', channel }, mode);

/** What the buttons do when nobody chose, then what the test says. */
function controlsView(
  given: Partial<Record<ButtonId, Partial<ButtonView>>> = {},
  times: Partial<Pick<ControlsView, 'longPressMs' | 'doublePressMs'>> = {},
): ControlsView {
  const starting: Partial<Record<ButtonId, Partial<ButtonView>>> = {
    fader1Mute: { short: under('a') },
    fader2Mute: { short: under('b') },
    fader3Mute: { short: under('c') },
    fader4Mute: { short: under('d') },
    micMute: { short: mute({ type: 'mic' }) },
    samplerSelectA: { short: { type: 'bank', bank: 'a' } },
    samplerSelectB: { short: { type: 'bank', bank: 'b' } },
    samplerSelectC: { short: { type: 'bank', bank: 'c' } },
  };
  return {
    longPressMs: 500,
    doublePressMs: 333,
    ...times,
    buttons: BUTTONS.map((button) => ({
      button,
      short: null,
      long: null,
      double: null,
      hold: null,
      ...(given[button] ?? starting[button]),
    })),
  };
}

function snapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  const band = (id: string) => ({
    band: id as MicView['equalizer'][number]['band'],
    frequency: 100,
    gain: 0,
    minFrequency: 30,
    maxFrequency: 300,
  });
  return {
    connection: { state: 'demo' },
    device: { kind: 'virtual', firmware: '1.4.3.110', serial: 'VIRTUAL' },
    faders: [
      { fader: 'a', channel: 'mic', volume: 255, muted: false },
      { fader: 'b', channel: 'chat', volume: 128, muted: false },
      { fader: 'c', channel: 'music', volume: 0, muted: false },
      { fader: 'd', channel: 'system', volume: 51, muted: false },
    ],
    channels: [],
    micOff: false,
    routing: [],
    pressed: [],
    micLevelDb: -30,
    mic: {
      micType: 'dynamic',
      gain: 30,
      gate: { threshold: -30, attenuation: 100, attack: 0, release: 19 },
      compressor: { threshold: 0, ratio: 9, attack: 1, release: 9, makeupGain: 0 },
      equalizer: [band('hz31')],
      deEsser: 0,
    },
    controls: controlsView(),
    touched: [],
    lastPress: null,
    bank: 'a',
    ...overrides,
  };
}

const real = { kind: 'hardware', firmware: '1.4.3.110', serial: 'S1' } as const;

async function report(state: unknown) {
  feed.push(state);
  await tick();
}

async function open(state: Snapshot = snapshot()) {
  render(App);
  await report(state);
  await fireEvent.click(screen.getByRole('button', { name: 'Controls' }));
}

const block = (name: string) => within(screen.getByRole('region', { name }));
const cell = (name: string) => screen.getByRole('button', { name: new RegExp(`^${name} ?:`) });

describe('Controls', () => {
  beforeEach(() => {
    i18n.setLocale('en');
    feed.sent.length = 0;
    feed.commands.length = 0;
    updates.reset();
  });

  describe('the screen', () => {
    it('shows the 24 buttons in four blocks, each saying what it does', async () => {
      await open();
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Controls');
      expect(block('Faders').getAllByRole('button')).toHaveLength(4);
      expect(block('Microphone').getAllByRole('button')).toHaveLength(2);
      expect(block('Effects').getAllByRole('button')).toHaveLength(10);
      expect(block('Sampler').getAllByRole('button')).toHaveLength(8);

      const named = (region: string) =>
        block(region)
          .getAllByRole('button')
          .map((button) => button.getAttribute('aria-label'));
      expect(named('Faders')).toEqual([
        'Mute A: Switch fader A',
        'Mute B: Switch fader B',
        'Mute C: Switch fader C',
        'Mute D: Switch fader D',
      ]);
      expect(named('Microphone')).toEqual(['Mic: Switch mic', 'Bleep: Nothing']);
      expect(named('Sampler')).toEqual([
        'Bank A: Bank A',
        'Bank B: Bank B',
        'Bank C: Bank C',
        'Pad top left: Nothing',
        'Pad top right: Nothing',
        'Pad bottom left: Nothing',
        'Pad bottom right: Nothing',
        'Clear: Nothing',
      ]);
      expect(named('Effects').every((label) => label?.endsWith(': Nothing'))).toBe(true);
    });

    it('says in a few words what a button with several actions does', async () => {
      await open(
        snapshot({
          controls: controlsView({
            samplerTopLeft: { long: track('music'), double: track('chat') },
            micMute: { hold: mute({ type: 'mic' }, 'mute') },
            samplerTopRight: { short: track('chat', 'unmute') },
          }),
        }),
      );
      expect(cell('Pad top left').getAttribute('aria-label')).toBe('Pad top left: Mute Music +1');
      expect(cell('Mic').getAttribute('aria-label')).toBe('Mic: Mute mic (held)');
      expect(cell('Pad top right').getAttribute('aria-label')).toBe('Pad top right: Open Chat');
    });

    it('waits for the device, and copes with a state that knows no controls yet', async () => {
      render(App);
      await fireEvent.click(screen.getByRole('button', { name: 'Controls' }));
      expect(screen.getByText('Connecting…')).toBeTruthy();

      const { controls: _, ...old } = snapshot();
      await report(old);
      expect(screen.getByText('Connecting…')).toBeTruthy();
      expect(screen.queryByRole('region', { name: 'Faders' })).toBeNull();
    });

    it('speaks French', async () => {
      await open();
      i18n.setLocale('fr');
      await tick();
      expect(screen.getByRole('heading', { level: 1 }).textContent).toBe('Touches');
      expect(cell('Coupure A').getAttribute('aria-label')).toBe(
        'Coupure A : Basculer le fader A',
      );
      expect(screen.getByRole('region', { name: 'Effets' })).toBeTruthy();
    });
  });

  describe('choosing what a gesture does', () => {
    it('asks to choose a button first, then shows the gestures of the one chosen', async () => {
      await open();
      expect(screen.getByText('Choose a button to set what it does.')).toBeTruthy();

      await fireEvent.click(cell('Pad top left'));
      expect(screen.queryByText('Choose a button to set what it does.')).toBeNull();
      const panel = within(screen.getByRole('region', { name: 'Pad top left' }));
      expect(panel.getAllByRole('tab').map((tab) => tab.textContent?.replace(/\s+/g, ' ').trim()))
        .toEqual([
          'Short press Nothing',
          'Long press Nothing',
          'Double press Nothing',
          'Hold Nothing',
        ]);
      expect(cell('Pad top left').getAttribute('aria-pressed')).toBe('true');
      expect(cell('Pad top right').getAttribute('aria-pressed')).toBe('false');
    });

    it('gives a gesture an audio action, then a target and what happens', async () => {
      await open();
      await fireEvent.click(cell('Pad top left'));
      await fireEvent.click(screen.getByRole('tab', { name: /Long press/ }));
      await fireEvent.change(screen.getByRole('combobox', { name: 'Action' }), {
        target: { value: 'audio' },
      });
      expect(feed.sent.at(-1)).toEqual({
        type: 'setGesture',
        button: 'samplerTopLeft',
        gesture: 'long',
        action: { type: 'mute', target: { type: 'mic' }, mode: 'toggle' },
      });

      // The device answers with the action in place: its settings appear.
      await report(
        snapshot({
          controls: controlsView({ samplerTopLeft: { long: mute({ type: 'mic' }) } }),
        }),
      );
      const target = screen.getByRole('combobox', { name: 'What to act on' }) as HTMLSelectElement;
      expect(target.value).toBe('mic');
      expect(within(target).getAllByRole('option').map((option) => option.textContent?.trim()))
        .toEqual([
          'The microphone',
          'The track under fader A',
          'The track under fader B',
          'The track under fader C',
          'The track under fader D',
          'Mic',
          'Line In',
          'Console',
          'System',
          'Game',
          'Chat',
          'Sampler',
          'Music',
          'Headphones',
          'Mic Monitor',
          'Line Out',
        ]);

      await fireEvent.change(target, { target: { value: 'channel:music' } });
      expect(feed.sent.at(-1)).toEqual({
        type: 'setGesture',
        button: 'samplerTopLeft',
        gesture: 'long',
        action: { type: 'mute', target: { type: 'channel', channel: 'music' }, mode: 'toggle' },
      });
      await fireEvent.change(screen.getByRole('combobox', { name: 'What to act on' }), {
        target: { value: 'fader:c' },
      });
      expect(feed.sent.at(-1)).toMatchObject({
        action: { target: { type: 'faderTrack', fader: 'c' } },
      });
      await fireEvent.change(screen.getByRole('combobox', { name: 'What happens' }), {
        target: { value: 'unmute' },
      });
      expect(feed.sent.at(-1)).toMatchObject({ action: { mode: 'unmute' } });
    });

    it('starts a fader button on the track under its fader', async () => {
      await open();
      await fireEvent.click(cell('Mute B'));
      await fireEvent.click(screen.getByRole('tab', { name: /Double press/ }));
      await fireEvent.change(screen.getByRole('combobox', { name: 'Action' }), {
        target: { value: 'audio' },
      });
      expect(feed.sent.at(-1)).toMatchObject({
        button: 'fader2Mute',
        gesture: 'double',
        action: { target: { type: 'faderTrack', fader: 'b' } },
      });
    });

    it('takes an action away, and offers a bank for a press but not for a hold', async () => {
      await open(snapshot({ controls: controlsView({ samplerTopLeft: { short: track('chat') } }) }));
      await fireEvent.click(cell('Pad top left'));
      await fireEvent.change(screen.getByRole('combobox', { name: 'Action' }), {
        target: { value: 'none' },
      });
      expect(feed.sent.at(-1)).toEqual({
        type: 'setGesture',
        button: 'samplerTopLeft',
        gesture: 'short',
        action: null,
      });
      const families = () =>
        within(screen.getByRole('combobox', { name: 'Action' }))
          .getAllByRole('option')
          .map((option) => option.textContent?.trim());
      expect(families()).toEqual(['Nothing', 'Audio', 'Pad bank']);

      await fireEvent.change(screen.getByRole('combobox', { name: 'Action' }), {
        target: { value: 'bank' },
      });
      expect(feed.sent.at(-1)).toMatchObject({ action: { type: 'bank', bank: 'a' } });

      await fireEvent.click(screen.getByRole('tab', { name: /Hold/ }));
      expect(families()).toEqual(['Nothing', 'Audio']);
    });

    it('chooses a bank', async () => {
      await open(
        snapshot({
          controls: controlsView({ samplerTopLeft: { short: { type: 'bank', bank: 'b' } } }),
        }),
      );
      await fireEvent.click(cell('Pad top left'));
      const bank = screen.getByRole('combobox', { name: 'Bank' }) as HTMLSelectElement;
      expect(bank.value).toBe('b');
      await fireEvent.change(bank, { target: { value: 'c' } });
      expect(feed.sent.at(-1)).toMatchObject({ action: { type: 'bank', bank: 'c' } });
    });

    it('says what a hold is, and that it clears the other gestures', async () => {
      await open(
        snapshot({
          controls: controlsView({ samplerTopLeft: { short: track('chat'), long: track('music') } }),
        }),
      );
      await fireEvent.click(cell('Pad top left'));
      expect(screen.queryByText(/reacts the moment you press, so/)).toBeNull();
      await fireEvent.click(screen.getByRole('tab', { name: /Hold/ }));
      expect(screen.getByText(/A hold reacts the moment you press/)).toBeTruthy();
      expect(screen.getByText(/removes the other gestures/)).toBeTruthy();

      await fireEvent.change(screen.getByRole('combobox', { name: 'Action' }), {
        target: { value: 'audio' },
      });
      expect(feed.sent.at(-1)).toEqual({
        type: 'setGesture',
        button: 'samplerTopLeft',
        gesture: 'hold',
        action: { type: 'mute', target: { type: 'mic' }, mode: 'mute' },
      });
    });

    it('words what happens in terms of the finger when it is a hold', async () => {
      await open(
        snapshot({
          controls: controlsView({ micMute: { hold: mute({ type: 'mic' }, 'mute') } }),
        }),
      );
      await fireEvent.click(cell('Mic'));
      await fireEvent.click(screen.getByRole('tab', { name: /Hold/ }));
      const mode = screen.getByRole('combobox', { name: 'What happens' });
      expect(within(mode).getAllByRole('option').map((option) => option.textContent?.trim()))
        .toEqual(['Muted while pressed', 'Open while pressed', 'Reversed while pressed']);
      // The hold does not clear anything when nothing else is set.
      expect(screen.queryByText(/removes the other gestures/)).toBeNull();
    });

    it('warns that a double press makes the short press wait', async () => {
      await open();
      await fireEvent.click(cell('Pad top left'));
      await fireEvent.click(screen.getByRole('tab', { name: /Double press/ }));
      expect(screen.getByText(/waits 333 ms/)).toBeTruthy();
      expect(screen.getByText('Two quick presses, less than 333 ms apart.')).toBeTruthy();

      await fireEvent.click(screen.getByRole('tab', { name: /Long press/ }));
      expect(screen.getByText('A press kept down for 500 ms.')).toBeTruthy();
    });

    it('tells a button that has a double press that its short press waits', async () => {
      await open(
        snapshot({
          controls: controlsView(
            { samplerTopLeft: { short: track('chat'), double: track('music') } },
            { doublePressMs: 250 },
          ),
        }),
      );
      await fireEvent.click(cell('Pad top left'));
      expect(screen.getByText(/short press of this button waits 250 ms/)).toBeTruthy();
    });

    it('puts one button, or all of them, back to default', async () => {
      await open();
      await fireEvent.click(cell('Pad top left'));
      await fireEvent.click(screen.getByRole('button', { name: 'Reset this button to default' }));
      expect(feed.sent.at(-1)).toEqual({ type: 'resetControls', button: 'samplerTopLeft' });
      await fireEvent.click(screen.getByRole('button', { name: 'Reset all buttons to default' }));
      expect(feed.sent.at(-1)).toEqual({ type: 'resetControls', button: null });
    });

    it('sets how long a press must last, and how long a second press is waited for', async () => {
      await open(snapshot({ controls: controlsView({}, { longPressMs: 600, doublePressMs: 300 }) }));
      const long = screen.getByRole('slider', { name: 'Long press' }) as HTMLInputElement;
      const double = screen.getByRole('slider', { name: 'Double press' }) as HTMLInputElement;
      expect(long.value).toBe('600');
      expect(long.getAttribute('aria-valuetext')).toBe('600 ms');
      expect(double.value).toBe('300');

      await fireEvent.input(long, { target: { value: '800' } });
      expect(feed.sent.at(-1)).toEqual({
        type: 'setPressTimes',
        longPressMs: 800,
        doublePressMs: 300,
      });
      await fireEvent.input(double, { target: { value: '200' } });
      expect(feed.sent.at(-1)).toEqual({
        type: 'setPressTimes',
        longPressMs: 600,
        doublePressMs: 200,
      });
    });
  });

  describe('pressing a real button', () => {
    it('selects its case', async () => {
      // The press was there before the screen looked: it is not one.
      await open(
        snapshot({
          connection: { state: 'hardware' },
          device: real,
          lastPress: { button: 'samplerBottomRight', count: 1 },
        }),
      );
      expect(screen.getByText('Choose a button to set what it does.')).toBeTruthy();
      await report(
        snapshot({
          connection: { state: 'hardware' },
          device: real,
          lastPress: { button: 'samplerBottomRight', count: 1 },
        }),
      );
      expect(screen.getByText('Choose a button to set what it does.')).toBeTruthy();

      await report(
        snapshot({
          connection: { state: 'hardware' },
          device: real,
          lastPress: { button: 'samplerBottomRight', count: 2 },
        }),
      );
      expect(cell('Pad bottom right').getAttribute('aria-pressed')).toBe('true');
      expect(screen.getByRole('region', { name: 'Pad bottom right' })).toBeTruthy();

      // The same button again, even a very short press, finds its case again.
      await fireEvent.click(cell('Mic'));
      expect(cell('Pad bottom right').getAttribute('aria-pressed')).toBe('false');
      await report(
        snapshot({
          connection: { state: 'hardware' },
          device: real,
          lastPress: { button: 'samplerBottomRight', count: 3 },
        }),
      );
      expect(cell('Pad bottom right').getAttribute('aria-pressed')).toBe('true');
    });

    it('does not select anything when no press came, whatever the state says', async () => {
      await open(snapshot({ connection: { state: 'hardware' }, device: real }));
      await report(snapshot({ connection: { state: 'hardware' }, device: real, lastPress: null }));
      await report(
        snapshot({
          connection: { state: 'hardware' },
          device: real,
          lastPress: { button: 'nowhere', count: 9 },
        }),
      );
      expect(screen.getByText('Choose a button to set what it does.')).toBeTruthy();
    });

    it('lights its case while it is held and for a moment after', async () => {
      await open(snapshot({ connection: { state: 'hardware' }, device: real }));
      expect(cell('Pad top left').classList.contains('lit')).toBe(false);
      await report(
        snapshot({
          connection: { state: 'hardware' },
          device: real,
          pressed: [],
          touched: ['samplerTopLeft'],
        }),
      );
      expect(cell('Pad top left').classList.contains('lit')).toBe(true);
      expect(cell('Pad top right').classList.contains('lit')).toBe(false);
      await report(snapshot({ connection: { state: 'hardware' }, device: real, touched: [] }));
      expect(cell('Pad top left').classList.contains('lit')).toBe(false);
    });

    it('falls back on the buttons held when the state does not tell the rest', async () => {
      const { touched: _, ...old } = snapshot({ pressed: ['micMute'] });
      await open(old as Snapshot);
      expect(cell('Mic').classList.contains('lit')).toBe(true);
    });
  });

  describe('without a GoXLR', () => {
    it('says the cases can be pressed', async () => {
      await open();
      expect(screen.getByText(/click a button to press it on the virtual device/)).toBeTruthy();
    });

    it('presses the button of the virtual device while the case is held, and selects it', async () => {
      await open();
      const pad = cell('Pad top left');
      await fireEvent.pointerDown(pad, { button: 0, pointerId: 1 });
      expect(feed.sent).toEqual([{ type: 'pressButton', button: 'samplerTopLeft', down: true }]);
      expect(pad.getAttribute('aria-pressed')).toBe('true');

      await fireEvent.pointerUp(pad, { button: 0, pointerId: 1 });
      expect(feed.sent.at(-1)).toEqual({
        type: 'pressButton',
        button: 'samplerTopLeft',
        down: false,
      });
      // Letting go is told once.
      await fireEvent.pointerLeave(pad);
      expect(feed.sent).toHaveLength(2);
    });

    it('lets go of the button when the pointer leaves the case', async () => {
      await open();
      const pad = cell('Pad top right');
      await fireEvent.pointerDown(pad, { button: 0, pointerId: 1 });
      await fireEvent.pointerLeave(pad);
      expect(feed.sent.at(-1)).toEqual({
        type: 'pressButton',
        button: 'samplerTopRight',
        down: false,
      });
    });

    it('presses and lets go on a click that no pointer came before', async () => {
      await open();
      await fireEvent.click(cell('Bleep'));
      expect(feed.sent).toEqual([
        { type: 'pressButton', button: 'bleep', down: true },
        { type: 'pressButton', button: 'bleep', down: false },
      ]);
      expect(cell('Bleep').getAttribute('aria-pressed')).toBe('true');
    });

    it('is pressed with the keyboard too, as long as the key is down', async () => {
      await open();
      const pad = cell('Pad bottom left');
      await fireEvent.keyDown(pad, { key: 'Enter' });
      await fireEvent.keyDown(pad, { key: 'Enter', repeat: true });
      await fireEvent.keyUp(pad, { key: 'Enter' });
      expect(feed.sent).toEqual([
        { type: 'pressButton', button: 'samplerBottomLeft', down: true },
        { type: 'pressButton', button: 'samplerBottomLeft', down: false },
      ]);
    });

    it('lets go of a held button when the screen is left', async () => {
      await open();
      await fireEvent.pointerDown(cell('Pad top left'), { button: 0, pointerId: 1 });
      await fireEvent.click(screen.getByRole('button', { name: 'Mixer' }));
      expect(feed.sent.at(-1)).toEqual({
        type: 'pressButton',
        button: 'samplerTopLeft',
        down: false,
      });
    });

    it('never presses anything with a real GoXLR: the cases only select', async () => {
      await open(snapshot({ connection: { state: 'hardware' }, device: real }));
      expect(screen.getByText(/Press a button on your GoXLR/)).toBeTruthy();
      const pad = cell('Pad top left');
      await fireEvent.pointerDown(pad, { button: 0, pointerId: 1 });
      await fireEvent.pointerUp(pad, { button: 0, pointerId: 1 });
      await fireEvent.keyDown(pad, { key: 'Enter' });
      await fireEvent.keyUp(pad, { key: 'Enter' });
      expect(feed.sent).toEqual([]);

      await fireEvent.click(pad);
      expect(pad.getAttribute('aria-pressed')).toBe('true');
      expect(feed.sent).toEqual([]);
    });
  });

  describe('on the table', () => {
    it('keeps a pad lit for a moment after a short press', async () => {
      render(App);
      await report(snapshot({ pressed: [], touched: ['samplerTopLeft'] }));
      const pad = screen.getByLabelText('Top left, released');
      expect(pad.classList.contains('down')).toBe(true);
      expect(screen.getByLabelText('Top right, released').classList.contains('down')).toBe(false);

      await report(snapshot({ pressed: ['samplerTopLeft'], touched: ['samplerTopLeft'] }));
      expect(screen.getByLabelText('Top left, pressed').classList.contains('down')).toBe(true);
    });
  });

  describe('with the profiles', () => {
    it('saves the controls like the other pieces', async () => {
      render(App);
      await report(
        snapshot({
          profiles: {
            active: { profile: 'Stream', mix: 'Desk', mic: 'Radio', controls: 'Keys' },
            profiles: ['Stream'],
            mixes: ['Desk'],
            mics: ['Radio'],
            controls: ['Keys', 'Pads'],
            dirty: { profile: false, mix: false, mic: false, controls: true },
            unsaved: true,
          },
        }),
      );
      expect(screen.getByRole('region', { name: 'Unsaved changes' })).toBeTruthy();
      await fireEvent.click(screen.getByRole('button', { name: 'Manage profiles' }));
      const card = within(screen.getByRole('region', { name: 'Controls' }));
      expect(card.getByRole('radio', { name: /Keys/ }).textContent).toContain('Not saved');
      await fireEvent.click(card.getByRole('button', { name: 'Save' }));
      expect(feed.commands).toEqual([{ type: 'save', kind: 'controls' }]);
    });
  });
});
