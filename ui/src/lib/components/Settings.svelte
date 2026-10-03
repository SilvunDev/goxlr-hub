<script lang="ts">
  import { i18n } from '../i18n/index.svelte';
  import { localeNames, locales } from '../i18n/locale';
</script>

<section>
  <h1>{i18n.t.nav.settings}</h1>
  <fieldset>
    <legend class="label">{i18n.t.settings.language}</legend>
    {#each locales as locale (locale)}
      <label>
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

  legend {
    margin-bottom: 10px;
    padding: 0;
  }

  label {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px 10px 12px;
    border: 1px solid var(--unlit);
    border-radius: 6px;
    background: var(--panel);
    color: var(--legend);
    cursor: pointer;
  }

  label:hover {
    color: var(--silkscreen);
  }

  /* The radio dot shows the choice; the amber border only reinforces it. */
  label:has(input:checked) {
    border-color: var(--amber);
    color: var(--silkscreen);
  }

  input {
    appearance: none;
    width: 14px;
    height: 14px;
    margin: 0;
    border: 2px solid var(--unlit);
    border-radius: 50%;
    cursor: pointer;
  }

  input:checked {
    border-color: var(--amber);
    background: radial-gradient(circle, var(--amber) 3px, transparent 3.5px);
  }
</style>
