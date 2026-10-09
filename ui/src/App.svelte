<script lang="ts">
  import { onMount } from 'svelte';
  import { onDeviceState, onQuitRequested, quit, runProfileCommand } from './lib/backend';
  import Channels from './lib/components/Channels.svelte';
  import ComingSoon from './lib/components/ComingSoon.svelte';
  import Controls from './lib/components/Controls.svelte';
  import Header from './lib/components/Header.svelte';
  import Mic from './lib/components/Mic.svelte';
  import Mixer from './lib/components/Mixer.svelte';
  import Profiles from './lib/components/Profiles.svelte';
  import QuitDialog from './lib/components/QuitDialog.svelte';
  import Routing from './lib/components/Routing.svelte';
  import Settings from './lib/components/Settings.svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import StatusBanner from './lib/components/StatusBanner.svelte';
  import UnsavedBanner from './lib/components/UnsavedBanner.svelte';
  import UpdateBanner from './lib/components/UpdateBanner.svelte';
  import {
    connectionOf,
    profilesOf,
    type ButtonId,
    type ProfileError,
    type Snapshot,
  } from './lib/device';
  import { i18n } from './lib/i18n/index.svelte';
  import type { SectionId } from './lib/nav';
  import { updates } from './lib/updates.svelte';

  let current = $state<SectionId>('mixer');
  // Replaced as a whole many times a second: no need to track its fields.
  let device = $state.raw<Snapshot | null>(null);
  /** The user asked to quit while something is not saved. */
  let quitting = $state(false);
  let saveError = $state<ProfileError | null>(null);
  /** A button the Mixer asked to see in the Controls screen. */
  let opened = $state<{ button: ButtonId; seq: number } | null>(null);

  const connection = $derived(device ? connectionOf(device) : null);
  const profiles = $derived(device ? profilesOf(device) : null);

  onMount(() => {
    const stopDevice = onDeviceState((snapshot) => (device = snapshot));
    const stopQuit = onQuitRequested(() => {
      saveError = null;
      quitting = true;
    });
    const stopUpdates = updates.start();
    return () => {
      stopDevice();
      stopQuit();
      stopUpdates();
    };
  });

  function openButton(button: ButtonId) {
    opened = { button, seq: (opened?.seq ?? 0) + 1 };
    current = 'controls';
  }

  function go(section: SectionId) {
    opened = null;
    current = section;
  }

  /** Saves the profile in use with its pieces. Says whether it was done. */
  async function save(): Promise<boolean> {
    saveError = await runProfileCommand({ type: 'save', kind: 'profile' });
    return saveError === null;
  }

  async function saveAndQuit() {
    if (await save()) await quit();
  }
</script>

<div class="frame">
  <Sidebar {current} onselect={go} />
  <div class="main">
    <div>
      {#if connection && connection.state !== 'hardware'}
        <StatusBanner {connection} />
      {/if}
      {#if profiles?.unsaved}
        <UnsavedBanner error={quitting ? null : saveError} onsave={save} />
      {/if}
      {#if updates.announced && current !== 'settings'}
        <UpdateBanner
          version={updates.announced.version}
          onsee={() => (current = 'settings')}
          onlater={() => updates.postpone()}
        />
      {/if}
      <Header
        {profiles}
        managing={current === 'profiles'}
        onmanage={() => (current = 'profiles')}
      />
    </div>
    <main>
      {#if current === 'settings'}
        <Settings unsaved={profiles?.unsaved ?? false} />
      {:else if current === 'profiles'}
        <Profiles {profiles} />
      {:else if current === 'mixer'}
        <Mixer {device} onopen={openButton} />
      {:else if current === 'mic'}
        <Mic {device} />
      {:else if current === 'channels'}
        <Channels {device} />
      {:else if current === 'routing'}
        <Routing {device} />
      {:else if current === 'controls'}
        <Controls {device} focus={opened} />
      {:else}
        <ComingSoon title={i18n.t.nav[current]} />
      {/if}
    </main>
  </div>
</div>

{#if quitting}
  <QuitDialog
    error={saveError}
    onsave={saveAndQuit}
    ondiscard={quit}
    oncancel={() => (quitting = false)}
  />
{/if}

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
