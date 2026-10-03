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

export type FaderId = 'a' | 'b' | 'c' | 'd';

export interface ChannelView {
  channel: ChannelId;
  /** 0 to 255. */
  volume: number;
  muted: boolean;
}

export interface FaderView extends ChannelView {
  fader: FaderId;
}

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
  /** Buttons held down right now. */
  pressed: string[];
  /** Between -72.2 (silence) and 0 (full scale). */
  micLevelDb: number;
}

/** A volume as a whole percentage. */
export function volumePercent(volume: number): number {
  const percent = Math.round((volume / 255) * 100);
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
