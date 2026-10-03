<script lang="ts">
  import ComingSoon from './lib/components/ComingSoon.svelte';
  import Header from './lib/components/Header.svelte';
  import Settings from './lib/components/Settings.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import { i18n } from './lib/i18n/index.svelte';
  import type { SectionId } from './lib/nav';

  let current = $state<SectionId>('mixer');
</script>

<div class="frame">
  <Sidebar {current} onselect={(section) => (current = section)} />
  <div class="main">
    <Header />
    <main>
      {#if current === 'settings'}
        <Settings />
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
