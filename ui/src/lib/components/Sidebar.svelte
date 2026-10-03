<script lang="ts">
  import logo from '../../../../assets/brand/logo-dark.svg';
  import { i18n } from '../i18n/index.svelte';
  import { sections, type SectionId } from '../nav';

  let { current, onselect }: { current: SectionId; onselect: (section: SectionId) => void } =
    $props();
</script>

{#snippet item(section: SectionId)}
  <button
    type="button"
    class="item"
    aria-current={current === section ? 'page' : undefined}
    onclick={() => onselect(section)}
  >
    <span class="dot" aria-hidden="true"></span>
    {i18n.t.nav[section]}
  </button>
{/snippet}

<nav aria-label={i18n.t.nav.label}>
  <img class="logo" src={logo} alt="GoXLR Hub" />
  <div class="sections">
    {#each sections as section (section)}
      {@render item(section)}
    {/each}
  </div>
  <div class="foot">
    {@render item('settings')}
    <p class="disclaimer">{i18n.t.disclaimer}</p>
  </div>
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 20px 12px 16px;
    background: var(--panel);
    border-right: 1px solid var(--unlit);
  }

  .logo {
    display: block;
    width: 168px;
    height: auto;
    margin: 0 8px 28px;
  }

  .sections {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    overflow: auto;
  }

  .foot {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--unlit);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 9px 10px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--legend);
    text-align: left;
    cursor: pointer;
  }

  .item:hover {
    color: var(--silkscreen);
    background: rgb(241 243 234 / 5%);
  }

  /* The active section lights its dot, like a pixel of the fader displays.
     Size, weight and background change too: amber never carries meaning alone. */
  .item[aria-current='page'] {
    color: var(--silkscreen);
    font-weight: 600;
    background: var(--case);
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--unlit);
    transform: scale(0.6);
  }

  .item[aria-current='page'] .dot {
    background: var(--amber);
    box-shadow: 0 0 8px rgb(255 178 36 / 55%);
    transform: none;
  }

  .disclaimer {
    padding: 0 10px;
    font-size: 11px;
    line-height: 1.4;
    color: var(--legend);
  }
</style>
