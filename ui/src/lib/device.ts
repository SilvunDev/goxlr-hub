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

/** What the interface asks of the device. The answer is the next snapshot. */
export type Intent =
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
