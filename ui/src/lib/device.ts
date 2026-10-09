// The picture of the device sent by the Rust side, many times a second.

export type ChannelId =
  | 'mic'
  | 'lineIn'
  | 'console'
  | 'system'
  | 'game'
  | 'chat'
  | 'sample'
  | 'music'
  | 'headphones'
  | 'micMonitor'
  | 'lineOut';

/** Every channel, in the numbering of the device. */
export const CHANNELS: readonly ChannelId[] = [
  'mic',
  'lineIn',
  'console',
  'system',
  'game',
  'chat',
  'sample',
  'music',
  'headphones',
  'micMonitor',
  'lineOut',
];

export type FaderId = 'a' | 'b' | 'c' | 'd';

export const MAX_VOLUME = 255;

export interface ChannelView {
  channel: ChannelId;
  /** 0 to 255, or `null` when the app does not know it. */
  volume: number | null;
  muted: boolean;
  /** The fader that carries the channel, if any. */
  fader: FaderId | null;
}

export interface FaderView {
  fader: FaderId;
  channel: ChannelId;
  /** 0 to 255. */
  volume: number;
  muted: boolean;
}

export type RoutingInputId =
  | 'mic'
  | 'chat'
  | 'music'
  | 'game'
  | 'console'
  | 'lineIn'
  | 'system'
  | 'samples';

/** The inputs, in the order of the rows of the routing grid. */
export const ROUTING_INPUTS: readonly RoutingInputId[] = [
  'mic',
  'chat',
  'music',
  'game',
  'console',
  'lineIn',
  'system',
  'samples',
];

export type RoutingOutputId = 'headphones' | 'broadcastMix' | 'chatMic' | 'sampler' | 'lineOut';

/** The outputs, in the order of the columns of the routing grid. */
export const ROUTING_OUTPUTS: readonly RoutingOutputId[] = [
  'headphones',
  'broadcastMix',
  'lineOut',
  'chatMic',
  'sampler',
];

export interface RouteView {
  input: RoutingInputId;
  /** The outputs the input is sent to. */
  outputs: RoutingOutputId[];
}

/** Two routes only feed a sound back to where it comes from. */
export function canRoute(input: RoutingInputId, output: RoutingOutputId): boolean {
  return !(
    (input === 'chat' && output === 'chatMic') ||
    (input === 'samples' && output === 'sampler')
  );
}

export type MicTypeId = 'dynamic' | 'condenser' | 'jack';

export const MIC_TYPES: readonly MicTypeId[] = ['dynamic', 'condenser', 'jack'];

/** The preamp gives 0 to 72 dB. */
export const MAX_GAIN_DB = 72;

export type GateSettingId = 'threshold' | 'attenuation' | 'attack' | 'release';

export type CompressorSettingId = 'threshold' | 'ratio' | 'attack' | 'release' | 'makeupGain';

export type EqBandId =
  | 'hz31'
  | 'hz63'
  | 'hz125'
  | 'hz250'
  | 'hz500'
  | 'khz1'
  | 'khz2'
  | 'khz4'
  | 'khz8'
  | 'khz16';

// The gate and the compressor take the rank of a time or of a ratio. These
// lists say what each rank stands for; they mirror the Rust side.

/** Attack and release times of the gate, in milliseconds. */
export const GATE_TIMES_MS: readonly number[] = [
  10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 250,
  300, 350, 400, 450, 500, 550, 600, 650, 700, 750, 800, 850, 900, 950, 1000, 1100, 1200, 1300,
  1400, 1500, 1600, 1700, 1800, 1900, 2000,
];

export const COMPRESSOR_RATIOS: readonly number[] = [
  1, 1.1, 1.2, 1.4, 1.6, 1.8, 2, 2.5, 3.2, 4, 5.6, 8, 16, 32, 64,
];

/** Attack times of the compressor, in milliseconds. */
export const COMPRESSOR_ATTACK_MS: readonly number[] = [
  0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 16, 18, 20, 23, 26, 30, 35, 40,
];

/** Release times of the compressor, in milliseconds. */
export const COMPRESSOR_RELEASE_MS: readonly number[] = [
  0, 15, 25, 35, 45, 55, 65, 75, 85, 100, 115, 140, 170, 230, 340, 680, 1000, 1500, 2000, 3000,
];

export interface EqBandView {
  band: EqBandId;
  /** In hertz. */
  frequency: number;
  /** -9 to 9 dB. */
  gain: number;
  /** How far the band can be moved, in hertz. */
  minFrequency: number;
  maxFrequency: number;
}

export interface MicView {
  /** `null` until it is chosen. */
  micType: MicTypeId | null;
  /** 0 to 72 dB, or `null` while no type is chosen. */
  gain: number | null;
  /** Threshold in dB, attenuation in percent, attack and release as ranks. */
  gate: Record<GateSettingId, number>;
  /** Threshold and make-up gain in dB, ratio, attack and release as ranks. */
  compressor: Record<CompressorSettingId, number>;
  /** One entry per band, lowest first. */
  equalizer: EqBandView[];
  /** 0 to 100. */
  deEsser: number;
}

/** A part of the microphone processing that can be put back to neutral. */
export type MicBlockId = 'gate' | 'compressor' | 'equalizer' | 'deEsser' | 'all';

/** What can be saved under a name: a profile, or one of the pieces it is made of. */
export type ProfileKind = 'profile' | 'mix' | 'mic' | 'controls';

export const PROFILE_KINDS: readonly ProfileKind[] = ['profile', 'mix', 'mic', 'controls'];

export interface ProfilesView {
  /** The names of what is in use. */
  active: Record<ProfileKind, string>;
  profiles: string[];
  mixes: string[];
  mics: string[];
  controls: string[];
  /** What differs from what is saved. `profile`: it is made of other pieces. */
  dirty: Record<ProfileKind, boolean>;
  /** Something was changed and not saved. */
  unsaved: boolean;
}

/** What the interface asks of the profiles. The answer says whether it was done. */
export type ProfileCommand =
  | { type: 'select'; kind: ProfileKind; name: string }
  | { type: 'save'; kind: ProfileKind }
  | { type: 'saveAs'; kind: ProfileKind; name: string }
  | { type: 'rename'; kind: ProfileKind; name: string; to: string }
  | { type: 'duplicate'; kind: ProfileKind; name: string; to: string }
  | { type: 'delete'; kind: ProfileKind; name: string };

export const PROFILE_ERRORS = [
  'invalidName',
  'nameTaken',
  'inUse',
  'notFound',
  'unreadable',
  'newer',
  'storage',
] as const;

/** Why a profile could not be saved, loaded or changed. */
export type ProfileError = (typeof PROFILE_ERRORS)[number];

/** The sampler buttons, as the device names them. */
export const PAD_BANKS = ['samplerSelectA', 'samplerSelectB', 'samplerSelectC'] as const;

export const PADS = [
  'samplerTopLeft',
  'samplerTopRight',
  'samplerBottomLeft',
  'samplerBottomRight',
] as const;

export const PAD_CLEAR = 'samplerClear';

export type PadId = (typeof PAD_BANKS)[number] | (typeof PADS)[number] | typeof PAD_CLEAR;

export type GestureId = 'short' | 'long' | 'double' | 'hold';

export const GESTURES: readonly GestureId[] = ['short', 'long', 'double', 'hold'];

export type BankId = 'a' | 'b' | 'c';

export const BANKS: readonly BankId[] = ['a', 'b', 'c'];

export type MuteMode = 'mute' | 'unmute' | 'toggle';

export const MUTE_MODES: readonly MuteMode[] = ['mute', 'unmute', 'toggle'];

/** What a mute action silences. */
export type AudioTarget =
  | { type: 'mic' }
  | { type: 'channel'; channel: ChannelId }
  | { type: 'faderTrack'; fader: FaderId };

/** What a route action does to a cell of the routing grid. `off` cuts it. */
export type RouteMode = 'off' | 'on' | 'toggle';

export const ROUTE_MODES: readonly RouteMode[] = ['off', 'on', 'toggle'];

export type VolumeMode = 'set' | 'up' | 'down';

export const VOLUME_MODES: readonly VolumeMode[] = ['set', 'up', 'down'];

/** What a gesture does. */
export type Action =
  | { type: 'mute'; target: AudioTarget; mode: MuteMode }
  | { type: 'route'; input: RoutingInputId; output: RoutingOutputId; mode: RouteMode }
  | { type: 'volume'; target: AudioTarget; mode: VolumeMode; percent: number }
  | { type: 'profile'; kind: ProfileKind; name: string }
  | { type: 'bank'; bank: BankId };

/** What a hold can be: only what lasts as long as the button is down. */
export function canHold(action: Action): boolean {
  return action.type === 'mute' || action.type === 'route';
}

export type WheelId = 'pitch' | 'gender' | 'reverb' | 'echo';

/** The four dials, left to right. */
export const WHEELS: readonly WheelId[] = ['pitch', 'gender', 'reverb', 'echo'];

/** What a dial does when it is turned. */
export interface WheelAction {
  type: 'volume';
  target: AudioTarget;
  /** Percent of the whole range, for each notch. */
  step: number;
}

export interface WheelView {
  wheel: WheelId;
  action: WheelAction | null;
}

/** What the app knows of a dial, for whoever looks into why one does not answer. */
export interface DialView {
  wheel: WheelId;
  /** The position the device reported at the last reading. */
  reading: number;
  state: 'idle' | 'waiting' | 'measuring' | 'ready' | 'followOnly';
  /** The travel found, once `ready`. */
  low: number | null;
  high: number | null;
  /** What the dial was last asked while it was measured. */
  asked: number | null;
  /** The device did not hear the last command to put the dial somewhere. */
  refused: boolean;
}

/** How far a notch of a dial moves a volume, in percent. */
export const WHEEL_STEP = { min: 1, max: 10, step: 1, start: 4 };

/** The 24 buttons of the GoXLR, in the order of the device. */
export const BUTTONS = [
  'effectSelect1',
  'effectSelect5',
  'samplerSelectA',
  'samplerTopLeft',
  'fader1Mute',
  'effectSelect2',
  'effectSelect6',
  'samplerSelectB',
  'samplerTopRight',
  'fader2Mute',
  'effectSelect3',
  'effectRobot',
  'samplerSelectC',
  'samplerBottomRight',
  'fader3Mute',
  'effectSelect4',
  'effectHardTune',
  'samplerBottomLeft',
  'samplerClear',
  'fader4Mute',
  'effectMegaphone',
  'effectFx',
  'bleep',
  'micMute',
] as const;

export type ButtonId = (typeof BUTTONS)[number];

/** What a button does, by gesture. A button with a hold has no other gesture. */
export interface ButtonView extends Record<GestureId, Action | null> {
  button: ButtonId;
}

export interface ControlsView {
  /** How long a press lasts to be a long one, in milliseconds. */
  longPressMs: number;
  /** How long a second press is waited for, in milliseconds. */
  doublePressMs: number;
  /** Every button, in the order of the device. */
  buttons: ButtonView[];
  /** Every dial, in the order of the device. */
  wheels?: WheelView[];
}

/** How long a press must last to be long, and how long a second press is waited for. */
export const LONG_PRESS_MS = { min: 200, max: 2000, step: 50 };
export const DOUBLE_PRESS_MS = { min: 150, max: 1000, step: 25 };

/** What the interface asks of the device. The answer is the next snapshot. */
export type Intent =
  | { type: 'setGesture'; button: ButtonId; gesture: GestureId; action: Action | null }
  | { type: 'resetControls'; button: ButtonId | null }
  | { type: 'setPressTimes'; longPressMs: number; doublePressMs: number }
  | { type: 'setWheel'; wheel: WheelId; action: WheelAction | null }
  | { type: 'pressButton'; button: ButtonId; down: boolean }
  | { type: 'turnWheel'; wheel: WheelId; notches: number }
  | { type: 'resetMic'; block: MicBlockId }
  | { type: 'setMicType'; micType: MicTypeId }
  | { type: 'setMicGain'; gain: number }
  | { type: 'setGate'; setting: GateSettingId; value: number }
  | { type: 'setCompressor'; setting: CompressorSettingId; value: number }
  | { type: 'setEqBand'; band: EqBandId; frequency: number; gain: number }
  | { type: 'setDeEsser'; amount: number }
  | { type: 'setVolume'; channel: ChannelId; volume: number }
  | { type: 'setMuted'; channel: ChannelId; muted: boolean }
  | { type: 'setMicOff'; off: boolean }
  | { type: 'assignFader'; fader: FaderId; channel: ChannelId }
  | { type: 'setRoute'; input: RoutingInputId; output: RoutingOutputId; on: boolean };

/**
 * Which device is shown, and why. Anything but `hardware` means the virtual
 * device is on screen.
 */
export type ConnectionState = 'demo' | 'hardware' | 'busy' | 'unsupported' | 'unreachable';

export interface Connection {
  state: ConnectionState;
  /** The program that drives the GoXLR, when `busy`. */
  program?: string;
}

export interface Snapshot {
  connection: Connection;
  device: {
    /** `virtual` is the simulated device of the demo mode. */
    kind: 'virtual' | 'hardware';
    firmware: string;
    serial: string;
  };
  faders: FaderView[];
  channels: ChannelView[];
  /** The microphone itself is off, whatever its channel says. */
  micOff: boolean;
  /** One row per input. */
  routing: RouteView[];
  /** Buttons held down right now. */
  pressed: string[];
  /** Between -72.2 (silence) and 0 (full scale). */
  micLevelDb: number;
  /** How the microphone is plugged in and processed. */
  mic: MicView;
  /** What is saved, what is in use and what changed since. */
  profiles?: ProfilesView;
  /** What each button does. */
  controls?: ControlsView;
  /** Buttons held down, or let go a moment ago. */
  touched?: string[];
  /** The button pressed last. `count` changes at every press. */
  lastPress?: { button: string; count: number } | null;
  /** The bank the pads are on. */
  bank?: BankId;
  /** What the app makes of each dial. */
  dials?: DialView[];
}

/** The profiles of a snapshot, or nothing when the Rust side did not tell them. */
export function profilesOf(snapshot: Snapshot): ProfilesView | null {
  const profiles = snapshot.profiles;
  const told =
    profiles &&
    profiles.active &&
    profiles.dirty &&
    [profiles.profiles, profiles.mixes, profiles.mics].every(Array.isArray) &&
    typeof profiles.active.profile === 'string' &&
    profiles.active.profile !== '';
  if (!told) return null;
  // A state that does not know the controls yet has none.
  return {
    ...profiles,
    controls: Array.isArray(profiles.controls) ? profiles.controls : [],
    active: { ...profiles.active, controls: profiles.active.controls ?? '' },
    dirty: { ...profiles.dirty, controls: profiles.dirty.controls ?? false },
  };
}

/** A volume as a whole percentage. */
export function volumePercent(volume: number): number {
  const percent = Math.round((volume / MAX_VOLUME) * 100);
  return Number.isFinite(percent) ? Math.min(100, Math.max(0, percent)) : 0;
}

const CONNECTION_STATES: readonly string[] = [
  'demo',
  'hardware',
  'busy',
  'unsupported',
  'unreachable',
];

/** The connection of a snapshot, whatever the Rust side sent. */
export function connectionOf(snapshot: Snapshot): Connection {
  const connection: Partial<Connection> | undefined = snapshot.connection;
  if (connection && CONNECTION_STATES.includes(connection.state ?? '')) {
    return connection as Connection;
  }
  if (!connection && snapshot.device.kind === 'hardware') return { state: 'hardware' };
  return { state: 'demo' };
}
