// What the Controls screen says of the buttons and of what they do.
import {
  BUTTONS,
  GESTURES,
  WHEEL_STEP,
  type Action,
  type AudioTarget,
  type ButtonId,
  type ButtonView,
  type ChannelId,
  type FaderId,
  type RoutingInputId,
  type RoutingOutputId,
  type WheelAction,
  type WheelView,
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

/**
 * What an audio action is given when the user picks its kind: a cut is the
 * only thing that is sure to be heard, a volume only goes down, a mute is the
 * one of the button.
 */
export function startingKind(kind: 'mute' | 'route' | 'volume', button: ButtonId, held: boolean): Action {
  if (kind === 'mute') return startingAudio(button, held);
  if (kind === 'route') {
    return { type: 'route', input: 'music', output: 'broadcastMix', mode: held ? 'off' : 'toggle' };
  }
  return {
    type: 'volume',
    target: { type: 'channel', channel: 'headphones' },
    mode: 'down',
    percent: 5,
  };
}

/** What a dial is given when the user picks a volume for it. */
export function startingWheel(): WheelAction {
  return { type: 'volume', target: { type: 'channel', channel: 'headphones' }, step: WHEEL_STEP.start };
}

type Texts = Messages['controls'];

/** The names an action is told with. */
export interface Names {
  channels: Record<ChannelId, string>;
  inputs: Record<RoutingInputId, string>;
  outputs: Record<RoutingOutputId, string>;
}

/** The names of the tracks and of the grid, in the language in use. */
export function namesOf(messages: Messages): Names {
  return {
    channels: messages.channels,
    inputs: messages.routing.inputs,
    outputs: messages.routing.outputs,
  };
}

function targetText(target: AudioTarget, t: Texts, names: Names): string {
  if (target.type === 'mic') return t.summary.mic;
  if (target.type === 'channel') return names.channels[target.channel];
  return t.summary.fader.replace('{fader}', target.fader.toUpperCase());
}

/** What an action does, in a few words. */
export function actionText(action: Action, t: Texts, names: Names, held = false): string {
  let text: string;
  switch (action.type) {
    case 'bank':
      text = t.summary.bank.replace('{bank}', action.bank.toUpperCase());
      break;
    case 'mute':
      text = t.summary[action.mode].replace('{target}', targetText(action.target, t, names));
      break;
    case 'route':
      text = t.summary.route[action.mode]
        .replace('{input}', names.inputs[action.input])
        .replace('{output}', names.outputs[action.output]);
      break;
    case 'volume':
      text = t.summary.volume[action.mode]
        .replace('{target}', targetText(action.target, t, names))
        .replace('{percent}', String(action.percent));
      break;
    case 'profile':
      text = t.summary.profile[action.kind].replace('{name}', action.name);
      break;
  }
  return held ? t.summary.held.replace('{action}', text) : text;
}

/** What a dial does, in a few words. */
export function wheelText(view: WheelView | undefined, t: Texts, names: Names): string {
  const action = view?.action;
  if (!action) return t.nothing;
  return t.summary.wheel
    .replace('{target}', targetText(action.target, t, names))
    .replace('{step}', String(action.step));
}

/**
 * What a button does, in two or three words: its first action, and how many
 * more it has.
 */
export function summary(view: ButtonView | undefined, t: Texts, names: Names): string {
  const given = GESTURES.filter((gesture) => view?.[gesture]);
  if (!view || given.length === 0) return t.nothing;
  const first = given[0];
  const text = actionText(view[first]!, t, names, first === 'hold');
  return given.length > 1 ? `${text} +${given.length - 1}` : text;
}

/** Whether a name is one of the buttons this version knows. */
export function isButton(name: string): name is ButtonId {
  return (BUTTONS as readonly string[]).includes(name);
}
