<script lang="ts">
  import type { EqBandId, EqBandView } from '../device';
  import {
    EQ_MAX_GAIN,
    curvePoints,
    frequencyToX,
    gainToY,
    nudgeFrequency,
    snapFrequency,
    xToFrequency,
    yToGain,
  } from '../eq';
  import { i18n } from '../i18n/index.svelte';

  let {
    bands,
    onmove,
  }: {
    bands: EqBandView[];
    onmove: (band: EqBandId, frequency: number, gain: number) => void;
  } = $props();

  // The drawing, in its own units: the plot, and room around it for the scales.
  const WIDTH = 720;
  const HEIGHT = 200;
  const LEFT = 34;
  const TOP = 14;
  const BOTTOM = 26;

  /** How long a point stays where it was put before the device is believed again. */
  const HOLD_MS = 300;

  const FREQUENCY_MARKS = [
    { hertz: 100, text: '100 Hz' },
    { hertz: 1000, text: '1 kHz' },
    { hertz: 10000, text: '10 kHz' },
  ];
  const GAIN_MARKS = [EQ_MAX_GAIN, 0, -EQ_MAX_GAIN];

  let svg = $state<SVGSVGElement>();
  // Points being moved on screen, shown instead of what the device reports.
  let held = $state<Partial<Record<EqBandId, { frequency: number; gain: number }>>>({});
  let timer: ReturnType<typeof setTimeout> | undefined;
  let dragged = $state<EqBandId | null>(null);

  let shown = $derived(bands.map((band) => ({ ...band, ...held[band.band] })));
  let line = $derived(
    curvePoints(shown, WIDTH, HEIGHT)
      .map((point, at) => `${at ? 'L' : 'M'}${point.x.toFixed(1)},${point.y.toFixed(1)}`)
      .join(' '),
  );

  function number(value: number): string {
    return value.toLocaleString(i18n.locale, { maximumFractionDigits: 1, useGrouping: false });
  }

  function told(band: { frequency: number; gain: number }): string {
    const gain = band.gain > 0 ? `+${band.gain}` : String(band.gain);
    return `${number(band.frequency)} Hz, ${gain} dB`;
  }

  function move(band: EqBandView, frequency: number, gain: number) {
    const bounded = {
      frequency: Math.min(band.maxFrequency, Math.max(band.minFrequency, frequency)),
      gain: Math.min(EQ_MAX_GAIN, Math.max(-EQ_MAX_GAIN, gain)),
    };
    if (bounded.frequency === band.frequency && bounded.gain === band.gain) return;
    held[band.band] = bounded;
    clearTimeout(timer);
    timer = setTimeout(() => (held = {}), HOLD_MS);
    onmove(band.band, bounded.frequency, bounded.gain);
  }

  function onkeydown(event: KeyboardEvent, band: EqBandView) {
    const { frequency, gain, minFrequency, maxFrequency } = band;
    if (event.key === 'ArrowUp') move(band, frequency, gain + 1);
    else if (event.key === 'ArrowDown') move(band, frequency, gain - 1);
    else if (event.key === 'ArrowRight')
      move(band, nudgeFrequency(frequency, 1, minFrequency, maxFrequency), gain);
    else if (event.key === 'ArrowLeft')
      move(band, nudgeFrequency(frequency, -1, minFrequency, maxFrequency), gain);
    else return;
    event.preventDefault();
  }

  function onpointerdown(event: PointerEvent, band: EqBandView) {
    dragged = band.band;
    (event.currentTarget as Element).setPointerCapture?.(event.pointerId);
  }

  function onpointermove(event: PointerEvent, band: EqBandView) {
    if (dragged !== band.band || !svg) return;
    const box = svg.getBoundingClientRect();
    if (!box.width || !box.height) return;
    const x = ((event.clientX - box.left) / box.width) * (LEFT + WIDTH) - LEFT;
    const y = ((event.clientY - box.top) / box.height) * (TOP + HEIGHT + BOTTOM) - TOP;
    move(band, snapFrequency(xToFrequency(x, WIDTH)), yToGain(y, HEIGHT));
  }
</script>

<svg
  bind:this={svg}
  viewBox="0 0 {LEFT + WIDTH} {TOP + HEIGHT + BOTTOM}"
  role="group"
  aria-label={i18n.t.mic.equalizer}
>
  <g transform="translate({LEFT} {TOP})">
    <g aria-hidden="true">
      <rect class="plot" width={WIDTH} height={HEIGHT} />
      {#each FREQUENCY_MARKS as mark (mark.hertz)}
        {@const x = frequencyToX(mark.hertz, WIDTH)}
        <line class="grid" x1={x} x2={x} y1="0" y2={HEIGHT} />
        <text class="scale" {x} y={HEIGHT + 18} text-anchor="middle">{mark.text}</text>
      {/each}
      {#each GAIN_MARKS as gain (gain)}
        {@const y = gainToY(gain, HEIGHT)}
        <line class="grid" class:zero={gain === 0} x1="0" x2={WIDTH} y1={y} y2={y} />
        <text class="scale" x="-8" y={y + 4} text-anchor="end">{gain > 0 ? `+${gain}` : gain}</text>
      {/each}
      <path class="curve" d={line} />
    </g>
    {#each shown as band, at (band.band)}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <g
        class="point"
        class:dragged={dragged === band.band}
        role="slider"
        tabindex="0"
        aria-label={i18n.t.mic.band.replace('{number}', String(at + 1))}
        aria-valuemin={-EQ_MAX_GAIN}
        aria-valuemax={EQ_MAX_GAIN}
        aria-valuenow={band.gain}
        aria-valuetext={told(band)}
        transform="translate({frequencyToX(band.frequency, WIDTH)} {gainToY(band.gain, HEIGHT)})"
        onkeydown={(event) => onkeydown(event, band)}
        onpointerdown={(event) => onpointerdown(event, band)}
        onpointermove={(event) => onpointermove(event, band)}
        onpointerup={() => (dragged = null)}
        onpointercancel={() => (dragged = null)}
      >
        <circle class="reach" r="15" />
        <circle class="dot" r="6.5" />
        <title>{told(band)}</title>
      </g>
    {/each}
  </g>
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    /* Dragging a point must not scroll the page. */
    touch-action: none;
  }

  .plot {
    fill: var(--case);
  }

  .grid {
    stroke: var(--unlit);
    stroke-width: 1;
    stroke-dasharray: 2 5;
  }

  .grid.zero {
    stroke: var(--legend);
    stroke-dasharray: none;
    opacity: 0.6;
  }

  .scale {
    fill: var(--legend);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .curve {
    fill: none;
    stroke: var(--amber);
    stroke-width: 2;
    stroke-linejoin: round;
  }

  .point {
    cursor: grab;
    outline: none;
  }

  .point.dragged {
    cursor: grabbing;
  }

  .reach {
    fill: transparent;
  }

  .dot {
    fill: var(--case);
    stroke: var(--silkscreen);
    stroke-width: 2;
  }

  .point:hover .dot,
  .point.dragged .dot {
    fill: var(--silkscreen);
  }

  .point:focus-visible .reach {
    stroke: var(--amber);
    stroke-width: 2;
  }
</style>
