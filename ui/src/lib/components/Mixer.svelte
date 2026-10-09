<script lang="ts">
  import { sendIntent } from '../backend';
  import type { Snapshot } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import FaderStrip from './FaderStrip.svelte';
  import MicMeter from './MicMeter.svelte';
  import MuteButton from './MuteButton.svelte';
  import Pads from './Pads.svelte';

  let { device }: { device: Snapshot | null } = $props();
</script>

<section class="mixer">
  <h1>{i18n.t.nav.mixer}</h1>
  {#if device}
    <div class="board">
      <div class="faders">
        {#each device.faders as view (view.fader)}
          <FaderStrip {view} />
        {/each}
      </div>
      <Pads
        pressed={Array.isArray(device.pressed) ? device.pressed : []}
        lit={Array.isArray(device.touched) ? device.touched : undefined}
      />
      <aside>
        <MicMeter levelDb={device.micLevelDb} />
        <div class="mic">
          <div>
            <strong>{i18n.t.mixer.mic}</strong>
            <p class="hint">{i18n.t.mixer.micHint}</p>
          </div>
          <MuteButton
            label={i18n.t.mixer.micOff}
            muted={device.micOff === true}
            ontoggle={(off) => sendIntent({ type: 'setMicOff', off })}
          />
        </div>
        <dl>
          <dt class="label">{i18n.t.device.title}</dt>
          <dd>{i18n.t.device[device.device.kind]}</dd>
          <dt class="label">{i18n.t.device.firmware}</dt>
          <dd class="mono">{device.device.firmware}</dd>
          <dt class="label">{i18n.t.device.serial}</dt>
          <dd class="mono">{device.device.serial}</dd>
        </dl>
      </aside>
    </div>
  {:else}
    <p class="hint">{i18n.t.mixer.connecting}</p>
  {/if}
</section>

<style>
  .mixer {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .hint {
    color: var(--legend);
  }

  .board {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 40px;
    margin-top: 18px;
  }

  .faders {
    display: flex;
    gap: 14px;
  }

  .mic {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .mic strong {
    font-weight: 600;
  }

  .mic .hint {
    font-size: 13px;
    line-height: 1.4;
  }

  aside {
    display: flex;
    flex-direction: column;
    gap: 28px;
    width: 252px;
  }

  dl {
    display: grid;
    gap: 4px;
    margin: 0;
    padding-top: 22px;
    border-top: 1px solid var(--unlit);
  }

  dd {
    margin: 0 0 12px;
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 13px;
    user-select: text;
  }
</style>
