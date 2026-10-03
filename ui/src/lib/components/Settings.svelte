<script lang="ts">
  import { onMount } from 'svelte';
  import { getStartup, setStartup, type Startup } from '../backend';
  import { i18n } from '../i18n/index.svelte';
  import { localeNames, locales } from '../i18n/locale';

  /** Nothing until the Rust side says how the app starts. */
  let startup = $state<Startup | null>(null);
  let failed = $state(false);

  onMount(async () => {
    startup = await getStartup();
  });

  /** The boxes show what the system says, never the click. */
  async function change(wanted: Startup) {
    const now = await setStartup(wanted);
    failed = now === null;
    if (now) startup = now;
  }
</script>

<section>
  <h1>{i18n.t.nav.settings}</h1>
  <fieldset>
    <legend class="label">{i18n.t.settings.language}</legend>
    {#each locales as locale (locale)}
      <label class="choice">
        <input
          type="radio"
          name="locale"
          value={locale}
          checked={i18n.locale === locale}
          onchange={() => i18n.setLocale(locale)}
        />
        <span lang={locale}>{localeNames[locale]}</span>
      </label>
    {/each}
  </fieldset>

  {#if startup}
    {@const { enabled, hidden } = startup}
    <fieldset class="stack">
      <legend class="label">{i18n.t.settings.startup}</legend>
      <label class="tick">
        <input
          type="checkbox"
          checked={enabled}
          onclick={(event) => {
            event.preventDefault();
            void change({ enabled: !enabled, hidden });
          }}
        />
        <span>{i18n.t.settings.startWithComputer}</span>
      </label>
      <label class="tick" class:off={!enabled}>
        <input
          type="checkbox"
          checked={hidden}
          disabled={!enabled}
          onclick={(event) => {
            event.preventDefault();
            void change({ enabled, hidden: !hidden });
          }}
        />
        <span>{i18n.t.settings.startHidden}</span>
      </label>
      {#if failed}
        <p role="alert">{i18n.t.settings.startupFailed}</p>
      {/if}
    </fieldset>
  {/if}
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: 28px;
    max-width: 560px;
  }

  fieldset {
    display: flex;
    gap: 10px;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .stack {
    flex-direction: column;
  }

  legend {
    margin-bottom: 10px;
    padding: 0;
  }

  label {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }

  .choice {
    padding: 10px 16px 10px 12px;
    border: 1px solid var(--unlit);
    border-radius: 6px;
    background: var(--panel);
    color: var(--legend);
  }

  .choice:hover {
    color: var(--silkscreen);
  }

  /* The radio dot shows the choice; the amber border only reinforces it. */
  .choice:has(input:checked) {
    border-color: var(--amber);
    color: var(--silkscreen);
  }

  .choice input {
    appearance: none;
    width: 14px;
    height: 14px;
    margin: 0;
    border: 2px solid var(--unlit);
    border-radius: 50%;
    cursor: pointer;
  }

  .choice input:checked {
    border-color: var(--amber);
    background: radial-gradient(circle, var(--amber) 3px, transparent 3.5px);
  }

  .tick input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--amber);
    cursor: pointer;
  }

  .off {
    color: var(--legend);
    cursor: default;
  }

  .off input {
    cursor: default;
  }

  p {
    font-weight: 600;
  }
</style>
