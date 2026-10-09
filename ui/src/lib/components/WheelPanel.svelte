<script lang="ts">
  import { sendIntent } from '../backend';
  import { startingWheel } from '../controls';
  import {
    type AudioTarget,
    type DialView,
    type WheelId,
    type WheelView,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import TargetSelect from './TargetSelect.svelte';

  let {
    wheel,
    name,
    view,
    dial = null,
  }: {
    wheel: WheelId;
    name: string;
    view: WheelView;
    /** What the app sees of the dial, when the device says. */
    dial?: DialView | null;
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

  function said(text: string, values: Record<string, string | number>): string {
    return Object.entries(values).reduce(
      (result, [key, value]) => result.replace(`{${key}}`, String(value)),
      text,
    );
  }

  /** How far a notch moves the volume: the whole volume over the notches of the travel. */
  const realStep = $derived.by(() => {
    if (!dial?.notches) return null;
    return Math.round((1000 / dial.notches)) / 10;
  });

  /** What the app sees of the dial, in words. */
  const seen = $derived.by(() => {
    if (!dial) return [];
    const d = t.wheel.diag;
    const lines = [said(d.reading, { reading: dial.reading })];
    if (dial.state === 'measuring') {
      lines.push(said(d.measuring, { asked: dial.asked ?? '—' }));
    } else if (dial.state === 'ready') {
      lines.push(
        said(d.ready, {
          low: dial.low ?? '—',
          high: dial.high ?? '—',
          notch: dial.notch ?? '—',
          notches: dial.notches ?? '—',
        }),
      );
    } else if (dial.state === 'followOnly') {
      lines.push(said(d.followOnly, { step: action?.step ?? '—' }));
    } else {
      lines.push(d[dial.state]);
    }
    if (dial.refused) lines.push(d.refused);
    return lines;
  });
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
    <p class="hint">
      {realStep === null
        ? t.wheel.stepUnknown
        : said(t.wheel.step, {
            step: realStep.toLocaleString(i18n.locale, { maximumFractionDigits: 1 }),
          })}
    </p>
    <p class="hint">{t.wheel.unknown}</p>
  {/if}

  {#if dial}
    <div class="seen" role="group" aria-label={t.wheel.diag.title}>
      <span class="label">{t.wheel.diag.title}</span>
      {#each seen as line (line)}
        <p class="hint">{line}</p>
      {/each}
    </div>
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

  .seen {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: 12px;
    border-top: 1px solid var(--unlit);
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
