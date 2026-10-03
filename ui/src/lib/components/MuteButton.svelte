<script lang="ts">
  import { sendIntent } from '../backend';
  import type { ChannelId } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { channel, muted }: { channel: ChannelId; muted: boolean } = $props();

  let label = $derived(i18n.t.mixer.mute.replace('{channel}', i18n.t.channels[channel]));
</script>

<button
  type="button"
  class="label"
  class:muted
  aria-label={label}
  aria-pressed={muted}
  onclick={() => sendIntent({ type: 'setMuted', channel, muted: !muted })}
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

  /* A muted channel says so in words and gets a filled badge. */
  .muted,
  .muted:hover {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
  }
</style>
