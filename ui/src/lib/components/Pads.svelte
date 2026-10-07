<script lang="ts">
  import { PAD_BANKS, PAD_CLEAR, PADS, type PadId } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { pressed, lit = pressed }: { pressed: readonly string[]; lit?: readonly string[] } =
    $props();

  /** What a screen reader says of a pad: its name, then whether it is held. */
  function said(pad: PadId, down: boolean): string {
    const text = down ? i18n.t.pads.pressed : i18n.t.pads.released;
    return text.replace('{pad}', i18n.t.pads.names[pad]);
  }
</script>

<!-- Shows what the device says, nothing to click: a list, not buttons. -->
{#snippet cell(pad: PadId, shape: string)}
  {@const down = pressed.includes(pad)}
  <!-- Lit while held and for a moment after, so that a quick press is seen. -->
  <li class={shape} class:down={lit.includes(pad)} aria-label={said(pad, down)}>
    <span aria-hidden="true">{i18n.t.pads.names[pad]}</span>
  </li>
{/snippet}

<section aria-label={i18n.t.pads.title}>
  <span class="label">{i18n.t.pads.title}</span>
  <ul class="banks">
    {#each PAD_BANKS as pad (pad)}
      {@render cell(pad, 'small')}
    {/each}
  </ul>
  <ul class="pads">
    {#each PADS as pad (pad)}
      {@render cell(pad, 'pad')}
    {/each}
  </ul>
  <ul class="banks">
    {@render cell(PAD_CLEAR, 'small')}
  </ul>
  <p class="hint">{i18n.t.pads.hint}</p>
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 220px;
  }

  ul {
    display: grid;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .banks {
    grid-template-columns: repeat(3, 1fr);
  }

  .pads {
    grid-template-columns: repeat(2, 1fr);
  }

  li {
    display: grid;
    place-items: center;
    padding: 4px;
    border: 1px solid var(--unlit);
    border-radius: 6px;
    color: var(--legend);
    font-size: 12px;
    text-align: center;
    line-height: 1.2;
  }

  .pad {
    aspect-ratio: 1;
  }

  .small {
    min-height: 30px;
  }

  /* A held pad is filled and its name gets heavier: never colour alone. */
  .down {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
    font-weight: 600;
  }

  .hint {
    margin-top: 4px;
    color: var(--legend);
    font-size: 13px;
    line-height: 1.4;
  }
</style>
