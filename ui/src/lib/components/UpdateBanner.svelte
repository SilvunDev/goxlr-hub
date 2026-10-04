<script lang="ts">
  import { i18n } from '../i18n/index.svelte';

  let {
    version,
    onsee,
    onlater,
  }: { version: string; onsee: () => void; onlater: () => void } = $props();
</script>

<div class="banner" role="region" aria-label={i18n.t.updates.tag}>
  <span class="label tag">{i18n.t.updates.tag}</span>
  <span class="body">{i18n.t.updates.available.replace('{version}', version)}</span>
  <button type="button" class="quiet" onclick={onlater}>{i18n.t.updates.later}</button>
  <button type="button" onclick={onsee}>{i18n.t.updates.see}</button>
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 48px;
    background: var(--panel);
    border-bottom: 1px solid var(--amber);
    font-size: 14px;
  }

  .tag {
    flex: none;
    padding: 4px 8px;
    border: 1px solid var(--amber);
    border-radius: 4px;
    color: var(--amber);
  }

  .body {
    flex: 1;
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

  button:hover,
  .quiet {
    background: none;
    color: var(--silkscreen);
  }

  .quiet {
    border-color: var(--unlit);
  }

  .quiet:hover {
    border-color: var(--silkscreen);
  }
</style>
