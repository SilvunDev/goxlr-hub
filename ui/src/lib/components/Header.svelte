<script lang="ts">
  import { i18n } from '../i18n/index.svelte';
</script>

<header>
  <span class="label">{i18n.t.profile.label}</span>
  <span class="value">{i18n.t.profile.none}</span>
</header>

<style>
  header {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 18px 48px;
    border-bottom: 1px solid var(--unlit);
  }

  .value {
    color: var(--legend);
  }
</style>
