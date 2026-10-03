<script lang="ts">
  import { i18n } from '../i18n/index.svelte';

  let {
    label,
    muted,
    ontoggle,
  }: { label: string; muted: boolean; ontoggle: (muted: boolean) => void } = $props();
</script>

<button
  type="button"
  class="label"
  class:muted
  aria-label={label}
  aria-pressed={muted}
  onclick={() => ontoggle(!muted)}
>
  {muted ? i18n.t.mixer.muted : i18n.t.mixer.live}
</button>

<style>
  button {
    min-width: 76px;
    padding: 6px 8px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  button:hover {
    border-color: var(--legend);
    color: var(--silkscreen);
  }

  /* What is muted says so in words and gets a filled badge. */
  .muted,
  .muted:hover {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
  }
</style>
