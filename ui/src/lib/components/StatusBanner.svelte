<script lang="ts">
  import type { Connection } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { connection }: { connection: Connection } = $props();

  // The real device needs no banner; shown with one anyway, it gets the
  // plainest.
  const text = $derived(
    i18n.t.connection[connection.state === 'hardware' ? 'demo' : connection.state],
  );
  const body = $derived(
    text.body.replace('{program}', connection.program || i18n.t.connection.busy.otherProgram),
  );
</script>

<div class="banner" role="status">
  <span class="label tag">{text.tag}</span>
  <span>{body}</span>
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
