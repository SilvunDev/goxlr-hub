<script lang="ts">
  import { i18n } from '../i18n/index.svelte';

  let { title }: { title: string } = $props();
</script>

<section>
  <span class="label tag">{i18n.t.comingSoon.tag}</span>
  <h1>{title}</h1>
  <p>{i18n.t.comingSoon.body}</p>
  <div class="matrix" aria-hidden="true"></div>
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 14px;
    max-width: 560px;
  }

  .tag {
    padding: 5px 9px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
  }

  p {
    color: var(--legend);
  }

  /* An unlit dot-matrix screen, waiting for content. */
  .matrix {
    width: 100%;
    height: 98px;
    margin-top: 18px;
    background-image: radial-gradient(circle, var(--unlit) 2px, transparent 2.5px);
    background-size: 14px 14px;
  }
</style>
