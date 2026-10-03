<script lang="ts">
  import { i18n } from '../i18n/index.svelte';
</script>

<div class="banner" role="status">
  <span class="label tag">{i18n.t.demo.tag}</span>
  <span>{i18n.t.demo.body}</span>
</div>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 48px;
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
</style>
