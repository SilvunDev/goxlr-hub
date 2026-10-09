<script lang="ts">
  import { sendIntent } from '../backend';
  import { CHANNELS, MAX_VOLUME, volumePercent, type ChannelId, type FaderView } from '../device';
  import { createHold } from '../hold.svelte';
  import { i18n } from '../i18n/index.svelte';
  import MuteButton from './MuteButton.svelte';

  let { view, onopen }: { view: FaderView; onopen?: () => void } = $props();

  /** Volume steps of the keyboard: about 1% and 10%. */
  const STEP = 3;
  const PAGE = 26;

  const hold = createHold();
  let dragging = $state(false);

  let volume = $derived(hold.value ?? view.volume);
  let percent = $derived(volumePercent(volume));
  let name = $derived(i18n.t.channels[view.channel]);

  function setVolume(next: number) {
    const clamped = Math.min(MAX_VOLUME, Math.max(0, Math.round(next)));
    if (clamped === volume) return;
    hold.set(clamped);
    sendIntent({ type: 'setVolume', channel: view.channel, volume: clamped });
  }

  /** The volume under the pointer: the top of the track is the maximum. */
  function follow(event: PointerEvent) {
    const track = (event.currentTarget as HTMLElement).getBoundingClientRect();
    if (track.height <= 0) return;
    setVolume((1 - (event.clientY - track.top) / track.height) * MAX_VOLUME);
  }

  function onpointerdown(event: PointerEvent) {
    if (event.button !== 0) return;
    dragging = true;
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    follow(event);
  }

  function onkeydown(event: KeyboardEvent) {
    const targets: Record<string, number> = {
      ArrowUp: volume + STEP,
      ArrowRight: volume + STEP,
      ArrowDown: volume - STEP,
      ArrowLeft: volume - STEP,
      PageUp: volume + PAGE,
      PageDown: volume - PAGE,
      Home: 0,
      End: MAX_VOLUME,
    };
    if (!(event.key in targets)) return;
    event.preventDefault();
    setVolume(targets[event.key]);
  }
</script>

<article
  class="strip"
  class:muted={view.muted}
  aria-label="{i18n.t.mixer.fader} {view.fader.toUpperCase()}"
>
  <span class="label">{view.fader}</span>
  <div
    class="track"
    class:dragging
    role="slider"
    tabindex="0"
    aria-label={name}
    aria-orientation="vertical"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={percent}
    aria-valuetext="{percent}%"
    {onpointerdown}
    onpointermove={(event) => dragging && follow(event)}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
    {onkeydown}
  >
    <div class="fill" style:height="{percent}%"></div>
    <div class="cap" style:bottom="{percent}%"></div>
  </div>
  <span class="value">{percent}%</span>
  <select
    aria-label={i18n.t.mixer.source.replace('{fader}', view.fader.toUpperCase())}
    value={view.channel}
    onchange={(event) =>
      sendIntent({
        type: 'assignFader',
        fader: view.fader,
        channel: event.currentTarget.value as ChannelId,
      })}
  >
    {#each CHANNELS as channel (channel)}
      <option value={channel}>{i18n.t.channels[channel]}</option>
    {/each}
  </select>
  <MuteButton
    label={i18n.t.mixer.mute.replace('{channel}', name)}
    muted={view.muted}
    ontoggle={(muted) => sendIntent({ type: 'setMuted', channel: view.channel, muted })}
  />
  {#if onopen}
    <button
      type="button"
      class="open"
      aria-label={i18n.t.mixer.openControls.replace('{fader}', view.fader.toUpperCase())}
      onclick={onopen}
    >
      {i18n.t.mixer.buttonSettings}
    </button>
  {/if}
</article>

<style>
  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 128px;
    padding: 16px 0 14px;
    background: var(--panel);
    border: 1px solid var(--unlit);
    border-radius: 10px;
  }

  /* The slot of the fader, drawn as two columns of dots that light up to
     the volume, like the LED ladders next to the real faders. The padding
     makes the whole width of the cap a handle. */
  .track {
    position: relative;
    box-sizing: content-box;
    width: 28px;
    height: 224px;
    margin: 10px 0;
    padding: 0 18px;
    background: radial-gradient(circle, var(--unlit) 2.5px, transparent 3px) left bottom / 14px
      14px content-box;
    cursor: grab;
    touch-action: none;
  }

  .track.dragging {
    cursor: grabbing;
  }

  .fill {
    position: absolute;
    inset: auto 18px 0;
    background: radial-gradient(circle, var(--amber) 2.5px, transparent 3px) left bottom / 14px
      14px;
    transition: height 80ms linear;
  }

  .cap {
    position: absolute;
    left: 50%;
    width: 46px;
    height: 20px;
    margin: 0 0 -10px -23px;
    border-radius: 4px;
    background: var(--silkscreen);
    box-shadow: 0 2px 6px rgb(0 0 0 / 45%);
    transition: bottom 80ms linear;
  }

  /* Under the hand, the cap follows at once. */
  .dragging .fill,
  .dragging .cap {
    transition: none;
  }

  .cap::after {
    content: '';
    position: absolute;
    inset: 9px 6px auto;
    height: 2px;
    background: var(--case);
  }

  .value {
    font-family: var(--font-mono);
    font-size: 13px;
  }

  select {
    width: 104px;
    padding: 5px 6px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: var(--case);
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .open {
    padding: 2px 6px;
    border: 0;
    background: none;
    color: var(--legend);
    font: inherit;
    font-size: 12px;
    text-decoration: underline;
    cursor: pointer;
  }

  .open:hover {
    color: var(--silkscreen);
  }

  /* A muted channel goes grey; its button says it in words. */
  .muted .fill {
    background-image: radial-gradient(circle, var(--legend) 2.5px, transparent 3px);
  }
</style>
