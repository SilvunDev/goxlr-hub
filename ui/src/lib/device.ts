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

/** What the interface asks of the device. The answer is the next snapshot. */
export type Intent =
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
