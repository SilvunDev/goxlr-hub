<script lang="ts">
  import type { ProfilesView } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let {
    profiles,
    managing,
    onmanage,
  }: { profiles: ProfilesView | null; managing: boolean; onmanage: () => void } = $props();
</script>

<header>
  <span class="label">{i18n.t.profile.label}</span>
  {#if profiles}
    <strong class="value">{profiles.active.profile}</strong>
  {:else}
    <span class="value none">{i18n.t.profile.none}</span>
  {/if}
  <button type="button" aria-current={managing ? 'page' : undefined} onclick={onmanage}>
    {i18n.t.profile.manage}
  </button>
</header>

<style>
  header {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 14px 48px;
    border-bottom: 1px solid var(--unlit);
  }

  .value {
    font-weight: 600;
    user-select: text;
  }

  .none {
    font-weight: 400;
    color: var(--legend);
  }

  button {
    margin-left: auto;
    padding: 6px 12px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    color: var(--legend);
    cursor: pointer;
  }

  button:hover {
    border-color: var(--legend);
    color: var(--silkscreen);
  }

  /* The open screen is said by the filled button, not by a colour. */
  button[aria-current='page'] {
    background: var(--panel);
    border-color: var(--legend);
    color: var(--silkscreen);
  }
</style>
