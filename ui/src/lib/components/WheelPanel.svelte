<script lang="ts">
  import { sendIntent } from '../backend';
  import { startingWheel } from '../controls';
  import { WHEEL_STEP, type AudioTarget, type WheelId, type WheelView } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import MicSlider from './MicSlider.svelte';
  import TargetSelect from './TargetSelect.svelte';

  let {
    wheel,
    name,
    view,
  }: {
    wheel: WheelId;
    name: string;
    view: WheelView;
  } = $props();

  const t = $derived(i18n.t.controls);
  const action = $derived(view.action);

  function give(next: WheelView['action']) {
    void sendIntent({ type: 'setWheel', wheel, action: next });
  }

  function pickAction(value: string) {
    give(value === 'volume' ? startingWheel() : null);
  }

  function pickTarget(target: AudioTarget) {
    if (action) give({ ...action, target });
  }

  function pickStep(step: number) {
    if (action) give({ ...action, step });
  }
</script>

<section class="panel" aria-label={name}>
  <h2>{name}</h2>
  <p class="hint">{t.wheel.hint}</p>

  <label class="field">
    <span class="label">{t.wheel.action}</span>
    <select
      value={action ? action.type : 'none'}
      onchange={(event) => pickAction(event.currentTarget.value)}
    >
      <option value="none">{t.wheel.none}</option>
      <option value="volume">{t.wheel.volume}</option>
    </select>
  </label>

  {#if action}
    <TargetSelect target={action.target} onchange={pickTarget} />
    <div class="amount">
      <MicSlider
        name={t.wheel.step}
        min={WHEEL_STEP.min}
        max={WHEEL_STEP.max}
        value={action.step}
        text={(step) => t.volume.percent.replace('{percent}', String(step))}
        onset={pickStep}
      />
    </div>
    <p class="hint">{t.wheel.unknown}</p>
  {/if}
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

  .hint {
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

  .amount :global(.row) {
    grid-template-columns: 120px minmax(60px, 1fr) 48px;
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
</style>
