<script lang="ts">
  import { onMount } from 'svelte';
  import { onDeviceState } from './lib/backend';
  import ComingSoon from './lib/components/ComingSoon.svelte';
  import DemoBanner from './lib/components/DemoBanner.svelte';
  import Header from './lib/components/Header.svelte';
  import Mixer from './lib/components/Mixer.svelte';
  import Settings from './lib/components/Settings.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import type { Snapshot } from './lib/device';
  import { i18n } from './lib/i18n/index.svelte';
  import type { SectionId } from './lib/nav';

  let current = $state<SectionId>('mixer');
  // Replaced as a whole many times a second: no need to track its fields.
  let device = $state.raw<Snapshot | null>(null);

  onMount(() => onDeviceState((snapshot) => (device = snapshot)));
</script>

<div class="frame">
  <Sidebar {current} onselect={(section) => (current = section)} />
  <div class="main">
    <div>
      {#if device?.device.kind === 'virtual'}
        <DemoBanner />
      {/if}
      <Header />
    </div>
    <main>
      {#if current === 'settings'}
        <Settings />
      {:else if current === 'mixer'}
        <Mixer {device} />
      {:else}
        <ComingSoon title={i18n.t.nav[current]} />
      {/if}
    </main>
  </div>
</div>

<style>
  .frame {
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
    height: 100%;
  }

  .main {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
  }

  main {
    overflow: auto;
    padding: 40px 48px;
  }
</style>
