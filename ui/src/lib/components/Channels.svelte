<script lang="ts">
  import type { Snapshot } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import ChannelRow from './ChannelRow.svelte';

  let { device }: { device: Snapshot | null } = $props();

  let someUnknown = $derived(device?.channels.some((view) => view.volume === null) ?? false);
</script>

<section>
  <h1>{i18n.t.nav.channels}</h1>
  {#if device}
    <p class="hint">{i18n.t.channelList.hint}</p>
    <ul>
      {#each device.channels as view (view.channel)}
        <ChannelRow {view} />
      {/each}
    </ul>
    {#if someUnknown}
      <p class="hint">{i18n.t.channelList.unknownHint}</p>
    {/if}
  {:else}
    <p class="hint">{i18n.t.mixer.connecting}</p>
  {/if}
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 760px;
  }

  .hint {
    color: var(--legend);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 18px 0 8px;
    padding: 0;
    list-style: none;
  }
</style>
