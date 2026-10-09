<script lang="ts">
  import { sendIntent } from '../backend';
  import { actionText, namesOf, startingAudio, startingKind } from '../controls';
  import {
    BANKS,
    GESTURES,
    MUTE_MODES,
    PROFILE_KINDS,
    ROUTE_MODES,
    ROUTING_INPUTS,
    ROUTING_OUTPUTS,
    VOLUME_MODES,
    canRoute,
    type Action,
    type AudioTarget,
    type BankId,
    type ButtonId,
    type ButtonView,
    type GestureId,
    type MuteMode,
    type ProfileKind,
    type ProfilesView,
    type RouteMode,
    type RoutingInputId,
    type RoutingOutputId,
    type VolumeMode,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import MicSlider from './MicSlider.svelte';
  import TargetSelect from './TargetSelect.svelte';

  let {
    button,
    name,
    view,
    doublePressMs,
    longPressMs,
    profiles = null,
  }: {
    button: ButtonId;
    name: string;
    view: ButtonView;
    doublePressMs: number;
    longPressMs: number;
    /** What can be switched to; nothing when the device does not tell. */
    profiles?: ProfilesView | null;
  } = $props();

  type Family = 'none' | 'audio' | 'profile' | 'bank';
  type AudioKind = 'mute' | 'route' | 'volume';

  let gesture = $state<GestureId>('short');

  const t = $derived(i18n.t.controls);
  const names = $derived(namesOf(i18n.t));
  const action = $derived(view[gesture]);
  const held = $derived(gesture === 'hold');
  const family = $derived<Family>(
    action === null
      ? 'none'
      : action.type === 'bank'
        ? 'bank'
        : action.type === 'profile'
          ? 'profile'
          : 'audio',
  );
  const audioKinds = $derived<readonly AudioKind[]>(
    held ? ['mute', 'route'] : ['mute', 'route', 'volume'],
  );
  const others = $derived(
    GESTURES.filter((other) => other !== 'hold' && view[other] !== null).length > 0,
  );

  /** The names a profile, or a piece of it, can be switched to. */
  const choices = $derived<Record<ProfileKind, string[]>>({
    profile: profiles?.profiles ?? [],
    mix: profiles?.mixes ?? [],
    mic: profiles?.mics ?? [],
    controls: profiles?.controls ?? [],
  });

  function give(next: Action | null) {
    void sendIntent({ type: 'setGesture', button, gesture, action: next });
  }

  /** What is in use now is a poor choice for a button: it would do nothing. */
  function startingProfile(): Action {
    const active = profiles?.active.profile ?? '';
    const list = choices.profile;
    return { type: 'profile', kind: 'profile', name: list.find((n) => n !== active) ?? list[0] ?? '' };
  }

  function pickFamily(next: Family) {
    if (next === 'none') give(null);
    else if (next === 'audio') give(startingAudio(button, held));
    else if (next === 'profile') give(startingProfile());
    else give({ type: 'bank', bank: 'a' });
  }

  function pickKind(kind: AudioKind) {
    give(startingKind(kind, button, held));
  }

  function pickTarget(target: AudioTarget) {
    if (action?.type === 'mute' || action?.type === 'volume') give({ ...action, target });
  }

  function pickMode(mode: MuteMode) {
    if (action?.type === 'mute') give({ ...action, mode });
  }

  function pickRouteMode(mode: RouteMode) {
    if (action?.type === 'route') give({ ...action, mode });
  }

  function pickInput(input: RoutingInputId) {
    if (action?.type !== 'route') return;
    // The new track may not be sent where the old one was.
    const output = canRoute(input, action.output)
      ? action.output
      : (ROUTING_OUTPUTS.find((other) => canRoute(input, other)) ?? action.output);
    give({ ...action, input, output });
  }

  function pickOutput(output: RoutingOutputId) {
    if (action?.type === 'route') give({ ...action, output });
  }

  function pickVolumeMode(mode: VolumeMode) {
    if (action?.type === 'volume') give({ ...action, mode });
  }

  function pickPercent(percent: number) {
    if (action?.type === 'volume') give({ ...action, percent });
  }

  function pickBank(bank: BankId) {
    give({ type: 'bank', bank });
  }

  function pickSwitchKind(kind: ProfileKind) {
    const active = profiles?.active[kind] ?? '';
    const list = choices[kind];
    give({ type: 'profile', kind, name: list.find((n) => n !== active) ?? list[0] ?? '' });
  }

  function pickSwitchName(next: string) {
    if (action?.type === 'profile') give({ ...action, name: next });
  }

  /** The profile an action names is gone: renamed, deleted. */
  const missing = $derived(
    action?.type === 'profile' && profiles !== null && !choices[action.kind].includes(action.name),
  );

  function said(text: string, values: Record<string, string | number>): string {
    return Object.entries(values).reduce(
      (result, [key, value]) => result.replace(`{${key}}`, String(value)),
      text,
    );
  }

  function tabText(other: GestureId): string {
    const given = view[other];
    return given ? actionText(given, t, names, other === 'hold') : t.nothing;
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
        {#if !held && profiles}
          <option value="profile">{t.families.profile}</option>
        {/if}
        {#if !held}
          <option value="bank">{t.families.bank}</option>
        {/if}
      </select>
    </label>

    {#if action && family === 'audio'}
      <label class="field">
        <span class="label">{t.audioKind}</span>
        <select
          value={action.type}
          onchange={(event) => pickKind(event.currentTarget.value as AudioKind)}
        >
          {#each audioKinds as kind (kind)}
            <option value={kind}>{t.audioKinds[kind]}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if action?.type === 'mute'}
      <TargetSelect target={action.target} onchange={pickTarget} />
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
    {:else if action?.type === 'route'}
      <p class="note">{t.route.hint}</p>
      <label class="field">
        <span class="label">{t.route.input}</span>
        <select
          value={action.input}
          onchange={(event) => pickInput(event.currentTarget.value as RoutingInputId)}
        >
          {#each ROUTING_INPUTS as input (input)}
            <option value={input}>{i18n.t.routing.inputs[input]}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span class="label">{t.route.output}</span>
        <select
          value={action.output}
          onchange={(event) => pickOutput(event.currentTarget.value as RoutingOutputId)}
        >
          {#each ROUTING_OUTPUTS.filter((output) => canRoute(action.input, output)) as output (output)}
            <option value={output}>{i18n.t.routing.outputs[output]}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span class="label">{t.mode}</span>
        <select
          value={action.mode}
          onchange={(event) => pickRouteMode(event.currentTarget.value as RouteMode)}
        >
          {#each ROUTE_MODES as mode (mode)}
            <option value={mode}>{held ? t.route.heldModes[mode] : t.route.modes[mode]}</option>
          {/each}
        </select>
      </label>
    {:else if action?.type === 'volume'}
      <TargetSelect target={action.target} onchange={pickTarget} />
      <label class="field">
        <span class="label">{t.mode}</span>
        <select
          value={action.mode}
          onchange={(event) => pickVolumeMode(event.currentTarget.value as VolumeMode)}
        >
          {#each VOLUME_MODES as mode (mode)}
            <option value={mode}>{t.volume.modes[mode]}</option>
          {/each}
        </select>
      </label>
      <div class="amount">
        <MicSlider
          name={t.volume.amount}
          min={0}
          max={100}
          value={action.percent}
          text={(percent) => said(t.volume.percent, { percent })}
          onset={pickPercent}
        />
      </div>
      <p class="note">{t.volume.hint}</p>
    {:else if action?.type === 'profile'}
      <label class="field">
        <span class="label">{t.profile.kind}</span>
        <select
          value={action.kind}
          onchange={(event) => pickSwitchKind(event.currentTarget.value as ProfileKind)}
        >
          {#each PROFILE_KINDS as kind (kind)}
            <option value={kind}>{t.profile.kinds[kind]}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span class="label">{t.profile.name}</span>
        <select
          value={action.name}
          disabled={choices[action.kind].length === 0 && !action.name}
          onchange={(event) => pickSwitchName(event.currentTarget.value)}
        >
          {#if missing || choices[action.kind].length === 0}
            <option value={action.name}>{action.name || t.profile.none}</option>
          {/if}
          {#each choices[action.kind] as other (other)}
            <option value={other}>{other}</option>
          {/each}
        </select>
      </label>
      {#if missing}
        <p class="note warning" role="alert">{said(t.profile.missing, { name: action.name })}</p>
      {/if}
      <p class="note">{t.profile.warning}</p>
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

  /* A warning is written in words and set apart by a bar: never colour alone. */
  .warning {
    padding-left: 10px;
    border-left: 4px solid var(--amber);
    color: var(--silkscreen);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .amount :global(.row) {
    grid-template-columns: 76px minmax(80px, 1fr) 56px;
    gap: 10px;
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
