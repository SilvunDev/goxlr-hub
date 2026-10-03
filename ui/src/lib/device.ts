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

export interface Snapshot {
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
