<script lang="ts">
  import type { ProfileError } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let {
    error,
    onsave,
    ondiscard,
    oncancel,
  }: {
    error: ProfileError | null;
    onsave: () => void;
    ondiscard: () => void;
    oncancel: () => void;
  } = $props();

  let first: HTMLButtonElement | undefined = $state();

  // The question takes the keyboard: Enter saves, Escape goes back.
  $effect(() => first?.focus());
</script>

<svelte:window onkeydown={(event) => event.key === 'Escape' && oncancel()} />

<div class="veil">
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="quit-title">
    <h2 id="quit-title">{i18n.t.quit.title}</h2>
    <p>{i18n.t.quit.body}</p>
    {#if error}
      <p class="error" role="alert">{i18n.t.profiles.errors[error]}</p>
    {/if}
    <div class="choices">
      <button type="button" class="main" bind:this={first} onclick={onsave}>
        {i18n.t.quit.save}
      </button>
      <button type="button" onclick={ondiscard}>{i18n.t.quit.discard}</button>
      <button type="button" onclick={oncancel}>{i18n.t.quit.cancel}</button>
    </div>
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 10;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 60%);
  }

  .dialog {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(480px, calc(100% - 48px));
    padding: 24px;
    background: var(--panel);
    border: 1px solid var(--legend);
    border-radius: 8px;
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }

  .error {
    font-weight: 600;
  }

  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }

  button {
    padding: 8px 14px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  button:hover {
    border-color: var(--legend);
  }

  .main {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
    font-weight: 600;
  }
</style>
