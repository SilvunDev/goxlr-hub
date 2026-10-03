<script lang="ts">
  import { createHold } from '../hold.svelte';

  let {
    name,
    label = name,
    min,
    max,
    value,
    text,
    disabled = false,
    onset,
  }: {
    /** What is written next to the slider. */
    name: string;
    /** What a screen reader calls it, when the name alone is ambiguous. */
    label?: string;
    min: number;
    max: number;
    /** `null` when there is nothing to show yet. */
    value: number | null;
    text: (value: number) => string;
    disabled?: boolean;
    onset: (value: number) => void;
  } = $props();

  const hold = createHold();

  let shown = $derived(hold.value ?? value);
  let told = $derived(shown === null ? '—' : text(shown));

  function oninput(event: Event & { currentTarget: HTMLInputElement }) {
    if (disabled) return;
    const next = Number(event.currentTarget.value);
    if (!Number.isInteger(next) || next < min || next > max) return;
    hold.set(next);
    onset(next);
  }
</script>

<div class="row" class:disabled>
  <span>{name}</span>
  <input
    type="range"
    {min}
    {max}
    {disabled}
    aria-label={label}
    aria-valuetext={told}
    value={shown ?? min}
    {oninput}
  />
  <span class="value">{told}</span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 150px minmax(120px, 1fr) 76px;
    align-items: center;
    gap: 16px;
  }

  input {
    width: 100%;
    accent-color: var(--amber);
    cursor: pointer;
  }

  .value {
    font-family: var(--font-mono);
    font-size: 13px;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  /* What cannot be set yet is drawn without colour. */
  .disabled {
    color: var(--legend);
  }

  .disabled input {
    accent-color: var(--legend);
    cursor: default;
  }
</style>
