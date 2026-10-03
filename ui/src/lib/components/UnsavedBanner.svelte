<script lang="ts">
  import type { ProfileError } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { error, onsave }: { error: ProfileError | null; onsave: () => void } = $props();
</script>

<div class="banner" role="region" aria-label={i18n.t.unsaved.tag}>
  <span class="label tag">{i18n.t.unsaved.tag}</span>
  <span class="body">
    {i18n.t.unsaved.body}
    {#if error}
      <strong role="alert">{i18n.t.profiles.errors[error]}</strong>
    {/if}
  </span>
  <button type="button" onclick={onsave}>{i18n.t.unsaved.save}</button>
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 48px;
    background: var(--panel);
    border-bottom: 1px solid var(--silkscreen);
    font-size: 14px;
  }

  .tag {
    flex: none;
    padding: 4px 8px;
    border: 1px solid var(--silkscreen);
    border-radius: 4px;
    color: var(--silkscreen);
  }

  .body {
    flex: 1;
  }

  strong {
    display: block;
    font-weight: 600;
  }

  button {
    flex: none;
    padding: 6px 16px;
    border: 1px solid var(--silkscreen);
    border-radius: 4px;
    background: var(--silkscreen);
    color: var(--case);
    font-weight: 600;
    cursor: pointer;
  }

  button:hover {
    background: none;
    color: var(--silkscreen);
  }
</style>
