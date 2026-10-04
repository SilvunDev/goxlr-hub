<script lang="ts">
  import {
    installVersion,
    openBackups,
    openRelease,
    setAutomaticUpdates,
    type PublishedVersion,
    type UpdateError,
  } from '../backend';
  import { i18n } from '../i18n/index.svelte';
  import { updates } from '../updates.svelte';

  /** Something is not saved: installing would lose it. */
  let { unsaved }: { unsaved: boolean } = $props();

  /** The version the user is asked to confirm. */
  let chosen = $state<string | null>(null);
  let installing = $state(false);
  let installError = $state<UpdateError | null>(null);

  const t = $derived(i18n.t.updates);
  const settings = $derived(updates.settings);

  function day(published: string): string {
    const date = new Date(published);
    return Number.isNaN(date.getTime()) ? '' : date.toLocaleDateString(i18n.locale);
  }

  async function setAutomatic(automatic: boolean) {
    const now = await setAutomaticUpdates(automatic);
    if (now) updates.settings = now;
  }

  function choose(version: PublishedVersion) {
    installError = null;
    chosen = version.version;
  }

  /** On success the app quits: nothing is left to show. */
  async function install(version: string) {
    installing = true;
    installError = await installVersion(version);
    installing = false;
  }
</script>

{#if settings}
  <fieldset>
    <legend class="label">{t.title}</legend>
    <p>
      {t.running.replace('{version}', settings.version)}
      {#if updates.versions && !updates.latest}
        <span class="quiet">{t.upToDate}</span>
      {/if}
    </p>
    <label class="tick">
      <input
        type="checkbox"
        checked={settings.automatic}
        onclick={(event) => {
          event.preventDefault();
          void setAutomatic(!settings.automatic);
        }}
      />
      <span>{t.automatic}</span>
    </label>
    <p class="quiet">{t.privacy}</p>
    <div class="row">
      <button type="button" disabled={updates.checking} onclick={() => updates.check(true)}>
        {updates.checking ? t.checking : t.check}
      </button>
      <button type="button" onclick={() => openBackups()}>{t.backups}</button>
    </div>
    {#if updates.error}
      <p role="alert">{t.errors[updates.error]}</p>
    {/if}

    {#if updates.versions}
      {#if updates.versions.length === 0}
        <p class="quiet">{t.none}</p>
      {:else}
        <ul aria-label={t.versions}>
          {#each updates.versions as version (version.version)}
            <li>
              <span class="number">{version.version}</span>
              <span class="quiet">{day(version.published)}</span>
              {#if version.current}
                <span class="label tag">{t.current}</span>
              {:else if version.newer}
                <span class="label tag new">{t.newer}</span>
              {/if}
              <span class="actions">
                <button type="button" class="link" onclick={() => openRelease(version.version)}>
                  {settings.installs ? t.notes : t.page}
                </button>
                {#if settings.installs && !version.current && version.installable}
                  <button type="button" onclick={() => choose(version)}>
                    {(version.newer ? t.install : t.goBack).replace('{version}', version.version)}
                  </button>
                {/if}
              </span>
              {#if chosen === version.version}
                <div class="confirm" role="group" aria-label={t.confirmTitle}>
                  <p>{t.confirm.replace('{version}', version.version)}</p>
                  {#if !version.newer}
                    <p>{t.confirmOlder}</p>
                  {/if}
                  {#if unsaved}
                    <p role="alert">{t.errors.unsaved}</p>
                  {:else if installError}
                    <p role="alert">{t.errors[installError]}</p>
                  {/if}
                  <div class="row">
                    <button
                      type="button"
                      class="primary"
                      disabled={unsaved || installing}
                      onclick={() => install(version.version)}
                    >
                      {installing ? t.installing : t.confirmInstall}
                    </button>
                    <button type="button" disabled={installing} onclick={() => (chosen = null)}>
                      {t.cancel}
                    </button>
                  </div>
                </div>
              {/if}
            </li>
          {/each}
        </ul>
        {#if !settings.installs}
          <p class="quiet">{t.packageManager}</p>
        {/if}
      {/if}
    {/if}
  </fieldset>
{/if}

<style>
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    margin-bottom: 10px;
    padding: 0;
  }

  p {
    margin: 0;
  }

  [role='alert'] {
    font-weight: 600;
  }

  .quiet {
    color: var(--legend);
    font-size: 14px;
  }

  .tick {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }

  .tick input {
    width: 16px;
    height: 16px;
    margin: 0;
    accent-color: var(--amber);
    cursor: pointer;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  button {
    padding: 6px 14px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: var(--panel);
    color: var(--silkscreen);
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--silkscreen);
  }

  button:disabled {
    color: var(--legend);
    cursor: default;
  }

  .primary:not(:disabled) {
    border-color: var(--amber);
    background: var(--amber);
    color: var(--case);
    font-weight: 600;
  }

  .link {
    border-color: transparent;
    background: none;
    color: var(--legend);
    text-decoration: underline;
  }

  ul {
    display: flex;
    flex-direction: column;
    margin: 6px 0 0;
    padding: 0;
    list-style: none;
    border-top: 1px solid var(--unlit);
  }

  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--unlit);
  }

  .number {
    min-width: 64px;
    font-family: var(--font-mono);
  }

  .tag {
    padding: 3px 7px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    color: var(--legend);
  }

  .new {
    border-color: var(--amber);
    color: var(--amber);
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }

  .confirm {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    padding: 14px 16px;
    border: 1px solid var(--amber);
    border-radius: 6px;
    background: var(--panel);
  }
</style>
