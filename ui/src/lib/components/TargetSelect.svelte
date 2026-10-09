<script lang="ts">
  import { CHANNELS, type AudioTarget, type ChannelId, type FaderId } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let {
    target,
    onchange,
  }: {
    target: AudioTarget;
    onchange: (target: AudioTarget) => void;
  } = $props();

  const FADERS: readonly FaderId[] = ['a', 'b', 'c', 'd'];

  const t = $derived(i18n.t.controls);

  /** A target as the text of an option, and back. */
  function encode(value: AudioTarget): string {
    if (value.type === 'mic') return 'mic';
    return value.type === 'channel' ? `channel:${value.channel}` : `fader:${value.fader}`;
  }

  function decode(text: string): AudioTarget {
    const [kind, rest] = text.split(':');
    if (kind === 'channel') return { type: 'channel', channel: rest as ChannelId };
    if (kind === 'fader') return { type: 'faderTrack', fader: rest as FaderId };
    return { type: 'mic' };
  }
</script>

<label class="field">
  <span class="label">{t.target}</span>
  <select value={encode(target)} onchange={(event) => onchange(decode(event.currentTarget.value))}>
    <option value="mic">{t.targetMic}</option>
    {#each FADERS as fader (fader)}
      <option value="fader:{fader}">
        {t.targetFader.replace('{fader}', fader.toUpperCase())}
      </option>
    {/each}
    <optgroup label={t.targetTracks}>
      {#each CHANNELS as channel (channel)}
        <option value="channel:{channel}">{i18n.t.channels[channel]}</option>
      {/each}
    </optgroup>
  </select>
</label>

<style>
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
</style>
