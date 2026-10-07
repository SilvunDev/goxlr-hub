// What the Controls screen says of the buttons and of what they do.
import {
  BUTTONS,
  GESTURES,
  type Action,
  type ButtonId,
  type ButtonView,
  type ChannelId,
  type FaderId,
} from './device';
import type { Messages } from './i18n/en';

export type BlockId = 'faders' | 'mic' | 'effects' | 'sampler';

/** The buttons of the device in blocks, as they sit on it. */
export const BLOCKS: readonly { id: BlockId; buttons: readonly ButtonId[] }[] = [
  {
    id: 'faders',
    buttons: ['fader1Mute', 'fader2Mute', 'fader3Mute', 'fader4Mute'],
  },
  { id: 'mic', buttons: ['micMute', 'bleep'] },
  {
    id: 'effects',
    buttons: [
      'effectSelect1',
      'effectSelect2',
      'effectSelect3',
      'effectSelect4',
      'effectSelect5',
      'effectSelect6',
      'effectFx',
      'effectMegaphone',
      'effectRobot',
      'effectHardTune',
    ],
  },
  {
    id: 'sampler',
    buttons: [
      'samplerSelectA',
      'samplerSelectB',
      'samplerSelectC',
      'samplerTopLeft',
      'samplerTopRight',
      'samplerBottomLeft',
      'samplerBottomRight',
      'samplerClear',
    ],
  },
];

/** The fader whose mute button this is, if it is one. */
export function faderOf(button: ButtonId): FaderId | null {
  const faders: Partial<Record<ButtonId, FaderId>> = {
    fader1Mute: 'a',
    fader2Mute: 'b',
    fader3Mute: 'c',
    fader4Mute: 'd',
  };
  return faders[button] ?? null;
}

/** What a button does when nothing is said of it. */
export function emptyView(button: ButtonId): ButtonView {
  return { button, short: null, long: null, double: null, hold: null };
}

/** What a mute action is given when the user picks "Audio" for a button. */
export function startingAudio(button: ButtonId, held: boolean): Action {
  const fader = faderOf(button);
  return {
    type: 'mute',
    target: fader ? { type: 'faderTrack', fader } : { type: 'mic' },
    mode: held ? 'mute' : 'toggle',
  };
}

type Texts = Messages['controls'];

function targetText(
  action: Extract<Action, { type: 'mute' }>,
  t: Texts,
  channels: Record<ChannelId, string>,
): string {
  const { target } = action;
  if (target.type === 'mic') return t.summary.mic;
  if (target.type === 'channel') return channels[target.channel];
  return t.summary.fader.replace('{fader}', target.fader.toUpperCase());
}

/** What an action does, in a few words. */
export function actionText(
  action: Action,
  t: Texts,
  channels: Record<ChannelId, string>,
  held = false,
): string {
  const text =
    action.type === 'bank'
      ? t.summary.bank.replace('{bank}', action.bank.toUpperCase())
      : t.summary[action.mode].replace('{target}', targetText(action, t, channels));
  return held ? t.summary.held.replace('{action}', text) : text;
}

/**
 * What a button does, in two or three words: its first action, and how many
 * more it has.
 */
export function summary(
  view: ButtonView | undefined,
  t: Texts,
  channels: Record<ChannelId, string>,
): string {
  const given = GESTURES.filter((gesture) => view?.[gesture]);
  if (!view || given.length === 0) return t.nothing;
  const first = given[0];
  const text = actionText(view[first]!, t, channels, first === 'hold');
  return given.length > 1 ? `${text} +${given.length - 1}` : text;
}

/** Whether a name is one of the buttons this version knows. */
export function isButton(name: string): name is ButtonId {
  return (BUTTONS as readonly string[]).includes(name);
}
