<script lang="ts">
  import { sendIntent } from '../backend';
  import {
    COMPRESSOR_ATTACK_MS,
    COMPRESSOR_RATIOS,
    COMPRESSOR_RELEASE_MS,
    GATE_TIMES_MS,
    MAX_GAIN_DB,
    MIC_TYPES,
    type CompressorSettingId,
    type GateSettingId,
    type MicBlockId,
    type Snapshot,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import EqCurve from './EqCurve.svelte';
  import MicMeter from './MicMeter.svelte';
  import MicSlider from './MicSlider.svelte';

  let { device }: { device: Snapshot | null } = $props();

  // A device that does not tell its microphone is waited for.
  let mic = $derived(
    device?.mic && Array.isArray(device.mic.equalizer) && device.mic.gate && device.mic.compressor
      ? device.mic
      : null,
  );

  const decibels = (value: number) => `${value} dB`;
  const percent = (value: number) => `${value}%`;
  const milliseconds = (times: readonly number[]) => (rank: number) => `${times[rank] ?? '?'} ms`;
  const ratio = (rank: number) =>
    `${(COMPRESSOR_RATIOS[rank] ?? 1).toLocaleString(i18n.locale, { maximumFractionDigits: 1 })}:1`;

  /** The name a screen reader gives a setting: its group, then its name. */
  function named(group: string, name: string): string {
    return i18n.t.mic.setting.replace('{group}', group).replace('{name}', name);
  }

  const gate = (setting: GateSettingId) => (value: number) =>
    sendIntent({ type: 'setGate', setting, value });
  const compressor = (setting: CompressorSettingId) => (value: number) =>
    sendIntent({ type: 'setCompressor', setting, value });
</script>

<!-- The way back from a setting that hurts the ears. -->
{#snippet head(title: string, block: MicBlockId)}
  <div class="head">
    <h2>{title}</h2>
    <button
      type="button"
      class="reset"
      aria-label={i18n.t.mic.resetNamed.replace('{group}', title)}
      onclick={() => sendIntent({ type: 'resetMic', block })}
    >
      {i18n.t.mic.reset}
    </button>
  </div>
{/snippet}

<section class="page">
  <h1>{i18n.t.nav.mic}</h1>
  {#if device && mic}
    <p class="hint">{i18n.t.mic.hint}</p>
    <div class="meter">
      <MicMeter levelDb={device.micLevelDb} />
      <button
        type="button"
        class="reset"
        onclick={() => sendIntent({ type: 'resetMic', block: 'all' })}
      >
        {i18n.t.mic.resetAll}
      </button>
    </div>

    <section class="card">
      <h2>{i18n.t.mic.input}</h2>
      <div class="types" role="radiogroup" aria-label={i18n.t.mic.type}>
        {#each MIC_TYPES as micType (micType)}
          <!-- A button, not an input: it shows what the device says, never the click. -->
          <button
            type="button"
            role="radio"
            aria-checked={mic.micType === micType}
            onclick={() => sendIntent({ type: 'setMicType', micType })}
          >
            {i18n.t.mic.types[micType]}
          </button>
        {/each}
      </div>
      {#if mic.micType === null}
        <p class="hint" role="note">{i18n.t.mic.chooseType}</p>
      {:else if mic.micType === 'condenser'}
        <p class="hint" role="note">{i18n.t.mic.phantom}</p>
      {/if}
      <MicSlider
        name={i18n.t.mic.gain}
        min={0}
        max={MAX_GAIN_DB}
        value={mic.gain}
        text={decibels}
        disabled={mic.micType === null}
        onset={(gain) => sendIntent({ type: 'setMicGain', gain })}
      />
    </section>

    <section class="card">
      {@render head(i18n.t.mic.gate, 'gate')}
      <p class="hint">{i18n.t.mic.gateHint}</p>
      <MicSlider
        name={i18n.t.mic.threshold}
        label={named(i18n.t.mic.gate, i18n.t.mic.threshold)}
        min={-59}
        max={0}
        value={mic.gate.threshold}
        text={decibels}
        onset={gate('threshold')}
      />
      <MicSlider
        name={i18n.t.mic.attenuation}
        label={named(i18n.t.mic.gate, i18n.t.mic.attenuation)}
        min={0}
        max={100}
        value={mic.gate.attenuation}
        text={percent}
        onset={gate('attenuation')}
      />
      <MicSlider
        name={i18n.t.mic.attack}
        label={named(i18n.t.mic.gate, i18n.t.mic.attack)}
        min={0}
        max={GATE_TIMES_MS.length - 1}
        value={mic.gate.attack}
        text={milliseconds(GATE_TIMES_MS)}
        onset={gate('attack')}
      />
      <MicSlider
        name={i18n.t.mic.release}
        label={named(i18n.t.mic.gate, i18n.t.mic.release)}
        min={0}
        max={GATE_TIMES_MS.length - 1}
        value={mic.gate.release}
        text={milliseconds(GATE_TIMES_MS)}
        onset={gate('release')}
      />
    </section>

    <section class="card">
      {@render head(i18n.t.mic.compressor, 'compressor')}
      <p class="hint">{i18n.t.mic.compressorHint}</p>
      <MicSlider
        name={i18n.t.mic.threshold}
        label={named(i18n.t.mic.compressor, i18n.t.mic.threshold)}
        min={-40}
        max={0}
        value={mic.compressor.threshold}
        text={decibels}
        onset={compressor('threshold')}
      />
      <MicSlider
        name={i18n.t.mic.ratio}
        label={named(i18n.t.mic.compressor, i18n.t.mic.ratio)}
        min={0}
        max={COMPRESSOR_RATIOS.length - 1}
        value={mic.compressor.ratio}
        text={ratio}
        onset={compressor('ratio')}
      />
      <MicSlider
        name={i18n.t.mic.attack}
        label={named(i18n.t.mic.compressor, i18n.t.mic.attack)}
        min={0}
        max={COMPRESSOR_ATTACK_MS.length - 1}
        value={mic.compressor.attack}
        text={milliseconds(COMPRESSOR_ATTACK_MS)}
        onset={compressor('attack')}
      />
      <MicSlider
        name={i18n.t.mic.release}
        label={named(i18n.t.mic.compressor, i18n.t.mic.release)}
        min={0}
        max={COMPRESSOR_RELEASE_MS.length - 1}
        value={mic.compressor.release}
        text={milliseconds(COMPRESSOR_RELEASE_MS)}
        onset={compressor('release')}
      />
      <MicSlider
        name={i18n.t.mic.makeupGain}
        label={named(i18n.t.mic.compressor, i18n.t.mic.makeupGain)}
        min={-6}
        max={24}
        value={mic.compressor.makeupGain}
        text={decibels}
        onset={compressor('makeupGain')}
      />
    </section>

    <section class="card">
      {@render head(i18n.t.mic.equalizer, 'equalizer')}
      <p class="hint">{i18n.t.mic.equalizerHint}</p>
      <EqCurve
        bands={mic.equalizer}
        onmove={(band, frequency, gain) =>
          sendIntent({ type: 'setEqBand', band, frequency, gain })}
      />
    </section>

    <section class="card">
      {@render head(i18n.t.mic.deEsser, 'deEsser')}
      <p class="hint">{i18n.t.mic.deEsserHint}</p>
      <MicSlider
        name={i18n.t.mic.amount}
        label={named(i18n.t.mic.deEsser, i18n.t.mic.amount)}
        min={0}
        max={100}
        value={mic.deEsser}
        text={percent}
        onset={(amount) => sendIntent({ type: 'setDeEsser', amount })}
      />
    </section>
  {:else}
    <p class="hint">{i18n.t.mixer.connecting}</p>
  {/if}
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 820px;
  }

  .hint {
    color: var(--legend);
  }

  .meter {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 16px 28px;
    margin: 18px 0 8px;
  }

  .meter > :global(:first-child) {
    flex: 1 1 320px;
    max-width: 420px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .reset {
    flex: none;
    padding: 5px 12px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    color: var(--legend);
    font-size: 13px;
    cursor: pointer;
  }

  .reset:hover {
    border-color: var(--legend);
    color: var(--silkscreen);
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px 20px;
    background: var(--panel);
    border: 1px solid var(--unlit);
    border-radius: 8px;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  .card .hint {
    margin-top: -8px;
    font-size: 13px;
  }

  .types {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .types button {
    padding: 7px 14px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  .types button:hover {
    border-color: var(--legend);
  }

  /* The type in use is filled and outlined: never colour alone. */
  .types button[aria-checked='true'] {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
    font-weight: 600;
  }

  .types + .hint {
    margin-top: 0;
  }
</style>
