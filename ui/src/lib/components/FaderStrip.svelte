<script lang="ts">
  import { volumePercent, type FaderView } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { view }: { view: FaderView } = $props();

  let percent = $derived(volumePercent(view.volume));
  let name = $derived(i18n.t.channels[view.channel]);
</script>

<article
  class="strip"
  class:muted={view.muted}
  aria-label="{i18n.t.mixer.fader} {view.fader.toUpperCase()}"
>
  <span class="label">{view.fader}</span>
  <div
    class="track"
    role="meter"
    aria-label={name}
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={percent}
  >
    <div class="fill" style:height="{percent}%"></div>
    <div class="cap" style:bottom="{percent}%"></div>
  </div>
  <span class="value">{percent}%</span>
  <strong class="name">{name}</strong>
  <span class="label state">{view.muted ? i18n.t.mixer.muted : i18n.t.mixer.live}</span>
</article>

<style>
  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    width: 112px;
    padding: 16px 0 14px;
    background: var(--panel);
    border: 1px solid var(--unlit);
    border-radius: 10px;
  }

  /* The slot of the fader, drawn as two columns of dots that light up to
     the volume, like the LED ladders next to the real faders. */
  .track {
    position: relative;
    width: 28px;
    height: 224px;
    margin: 10px 0;
    background: radial-gradient(circle, var(--unlit) 2.5px, transparent 3px) left bottom / 14px
      14px;
  }

  .fill {
    position: absolute;
    inset: auto 0 0;
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

  .name {
    font-weight: 600;
  }

  .state {
    padding: 4px 8px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
  }

  /* A muted channel says so in words, goes grey and gets a filled badge. */
  .muted .fill {
    background-image: radial-gradient(circle, var(--legend) 2.5px, transparent 3px);
  }

  .muted .state {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
  }
</style>
