<script lang="ts">
  import { sendIntent } from '../backend';
  import { actionText, startingAudio } from '../controls';
  import {
    BANKS,
    CHANNELS,
    GESTURES,
    MUTE_MODES,
    type Action,
    type AudioTarget,
    type BankId,
    type ButtonId,
    type ButtonView,
    type ChannelId,
    type FaderId,
    type GestureId,
    type MuteMode,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let {
    button,
    name,
    view,
    doublePressMs,
    longPressMs,
  }: {
    button: ButtonId;
    name: string;
    view: ButtonView;
    doublePressMs: number;
    longPressMs: number;
  } = $props();

  type Family = 'none' | 'audio' | 'bank';

  const FADERS: readonly FaderId[] = ['a', 'b', 'c', 'd'];

  let gesture = $state<GestureId>('short');

  const t = $derived(i18n.t.controls);
  const action = $derived(view[gesture]);
  const held = $derived(gesture === 'hold');
  const family = $derived<Family>(
    action === null ? 'none' : action.type === 'mute' ? 'audio' : 'bank',
  );
  const others = $derived(
    GESTURES.filter((other) => other !== 'hold' && view[other] !== null).length > 0,
  );

  function give(next: Action | null) {
    void sendIntent({ type: 'setGesture', button, gesture, action: next });
  }

  function pickFamily(next: Family) {
    if (next === 'none') give(null);
    else if (next === 'audio') give(startingAudio(button, held));
    else give({ type: 'bank', bank: 'a' });
  }

  /** A target as the text of an option, and back. */
  function encode(target: AudioTarget): string {
    if (target.type === 'mic') return 'mic';
    return target.type === 'channel' ? `channel:${target.channel}` : `fader:${target.fader}`;
  }

  function decode(text: string): AudioTarget {
    const [kind, rest] = text.split(':');
    if (kind === 'channel') return { type: 'channel', channel: rest as ChannelId };
    if (kind === 'fader') return { type: 'faderTrack', fader: rest as FaderId };
    return { type: 'mic' };
  }

  function pickTarget(text: string) {
    if (action?.type === 'mute') give({ ...action, target: decode(text) });
  }

  function pickMode(mode: MuteMode) {
    if (action?.type === 'mute') give({ ...action, mode });
  }

  function pickBank(bank: BankId) {
    give({ type: 'bank', bank });
  }

  function said(text: string, values: Record<string, string | number>): string {
    return Object.entries(values).reduce(
      (result, [key, value]) => result.replace(`{${key}}`, String(value)),
      text,
    );
  }

  function tabText(other: GestureId): string {
    const given = view[other];
    return given ? actionText(given, t, i18n.t.channels, other === 'hold') : t.nothing;
  }

  function hint(other: GestureId): string {
    const ms = other === 'long' ? longPressMs : doublePressMs;
    return said(t.gestureHints[other], { ms });
  }
</script>

<section class="panel" aria-label={name}>
  <h2>{name}</h2>

  <div class="tabs" role="tablist" aria-label={name}>
    {#each GESTURES as other (other)}
      <button
        type="button"
        role="tab"
        id="gesture-{other}"
        aria-selected={gesture === other}
        aria-controls="gesture-editor"
        class:current={gesture === other}
        onclick={() => (gesture = other)}
      >
        <span class="label">{t.gestures[other]}</span>
        <span class="given">{tabText(other)}</span>
      </button>
    {/each}
  </div>

  <div class="editor" id="gesture-editor" role="tabpanel" aria-labelledby="gesture-{gesture}">
    <p class="hint">{hint(gesture)}</p>

    <label class="field">
      <span class="label">{t.family}</span>
      <select value={family} onchange={(event) => pickFamily(event.currentTarget.value as Family)}>
        <option value="none">{t.families.none}</option>
        <option value="audio">{t.families.audio}</option>
        {#if !held}
          <option value="bank">{t.families.bank}</option>
        {/if}
      </select>
    </label>

    {#if action?.type === 'mute'}
      <label class="field">
        <span class="label">{t.target}</span>
        <select
          value={encode(action.target)}
          onchange={(event) => pickTarget(event.currentTarget.value)}
        >
          <option value="mic">{t.targetMic}</option>
          {#each FADERS as fader (fader)}
            <option value="fader:{fader}">
              {said(t.targetFader, { fader: fader.toUpperCase() })}
            </option>
          {/each}
          <optgroup label={t.targetTracks}>
            {#each CHANNELS as channel (channel)}
              <option value="channel:{channel}">{i18n.t.channels[channel]}</option>
            {/each}
          </optgroup>
        </select>
      </label>
      <label class="field">
        <span class="label">{t.mode}</span>
        <select
          value={action.mode}
          onchange={(event) => pickMode(event.currentTarget.value as MuteMode)}
        >
          {#each MUTE_MODES as mode (mode)}
            <option value={mode}>{held ? t.heldModes[mode] : t.modes[mode]}</option>
          {/each}
        </select>
      </label>
    {:else if action?.type === 'bank'}
      <label class="field">
        <span class="label">{t.bank}</span>
        <select
          value={action.bank}
          onchange={(event) => pickBank(event.currentTarget.value as BankId)}
        >
          {#each BANKS as bank (bank)}
            <option value={bank}>{bank.toUpperCase()}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if gesture === 'hold'}
      <p class="note">{t.holdNote}</p>
      {#if others}
        <p class="note">{t.holdClears}</p>
      {/if}
    {:else if gesture === 'double' || (gesture === 'short' && view.double)}
      <p class="note">{said(t.doubleNote, { ms: doublePressMs })}</p>
    {:else if gesture === 'short' && view.long}
      <p class="note">{t.longNote}</p>
    {/if}
  </div>

  <button
    type="button"
    class="reset"
    onclick={() => sendIntent({ type: 'resetControls', button })}
  >
    {t.resetButton}
  </button>
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding: 18px;
    border: 1px solid var(--unlit);
    border-radius: 8px;
    background: var(--panel);
  }

  h2 {
    margin: 0;
    font-size: 18px;
  }

  .tabs {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  [role='tab'] {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border: 1px solid var(--unlit);
    border-radius: 6px;
    background: none;
    color: var(--legend);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  [role='tab']:hover {
    border-color: var(--legend);
  }

  [role='tab'] .label {
    color: var(--silkscreen);
  }

  /* The open tab has a bar on its edge and a thicker line: never colour alone. */
  .current {
    border-color: var(--amber);
    border-left-width: 5px;
    padding-left: 6px;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .hint,
  .note {
    margin: 0;
    color: var(--legend);
    font-size: 13px;
    line-height: 1.4;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  select {
    padding: 6px 8px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: var(--case);
    color: var(--silkscreen);
    font: inherit;
  }

  .reset {
    align-self: flex-start;
    padding: 6px 10px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  .reset:hover {
    border-color: var(--legend);
  }
</style>
