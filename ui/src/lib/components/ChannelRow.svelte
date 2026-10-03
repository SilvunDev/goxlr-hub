<script lang="ts">
  import { sendIntent } from '../backend';
  import { MAX_VOLUME, volumePercent, type ChannelView } from '../device';
  import { createHold } from '../hold.svelte';
  import { i18n } from '../i18n/index.svelte';
  import MuteButton from './MuteButton.svelte';

  let { view }: { view: ChannelView } = $props();

  const hold = createHold();

  let volume = $derived(hold.value ?? view.volume);
  let name = $derived(i18n.t.channels[view.channel]);

  function oninput(event: Event & { currentTarget: HTMLInputElement }) {
    const next = Number(event.currentTarget.value);
    if (!Number.isInteger(next) || next < 0 || next > MAX_VOLUME) return;
    hold.set(next);
    sendIntent({ type: 'setVolume', channel: view.channel, volume: next });
  }
</script>

<li class:muted={view.muted}>
  <strong>{name}</strong>
  <span class="label">
    {#if view.fader}
      {i18n.t.mixer.fader} {view.fader.toUpperCase()}
    {/if}
  </span>
  <input
    type="range"
    min="0"
    max={MAX_VOLUME}
    aria-label={name}
    aria-valuetext={volume === null ? i18n.t.channelList.unknown : `${volumePercent(volume)}%`}
    class:unknown={volume === null}
    value={volume ?? 0}
    {oninput}
  />
  <span class="value">
    {volume === null ? i18n.t.channelList.unknown : `${volumePercent(volume)}%`}
  </span>
  <MuteButton channel={view.channel} muted={view.muted} />
</li>

<style>
  li {
    display: grid;
    grid-template-columns: 120px 64px minmax(120px, 1fr) 76px auto;
    align-items: center;
    gap: 16px;
    padding: 10px 14px 10px 18px;
    background: var(--panel);
    border: 1px solid var(--unlit);
    border-radius: 8px;
  }

  strong {
    font-weight: 600;
  }

  input {
    width: 100%;
    accent-color: var(--amber);
    cursor: pointer;
  }

  /* A volume nobody knows is drawn without colour, and said in words. */
  input.unknown,
  .muted input {
    accent-color: var(--legend);
  }

  .value {
    font-family: var(--font-mono);
    font-size: 13px;
    text-align: right;
  }
</style>
