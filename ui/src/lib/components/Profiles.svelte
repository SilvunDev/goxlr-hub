<script lang="ts">
  import { tick } from 'svelte';
  import { runProfileCommand } from '../backend';
  import {
    PROFILE_KINDS,
    type ProfileCommand,
    type ProfileError,
    type ProfileKind,
    type ProfilesView,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';

  let { profiles }: { profiles: ProfilesView | null } = $props();

  /** What is being asked before it is done: a name to type, or a yes. */
  type Pending =
    | { kind: ProfileKind; action: 'saveAs' }
    | { kind: ProfileKind; action: 'rename' | 'duplicate' | 'delete' | 'select'; name: string };

  let pending = $state<Pending | null>(null);
  let draft = $state('');
  /** The last refusal, shown in the card it belongs to. */
  let failed = $state<{ kind: ProfileKind; error: ProfileError } | null>(null);
  let field: HTMLInputElement | undefined = $state();

  function names(view: ProfilesView, kind: ProfileKind): string[] {
    return kind === 'profile' ? view.profiles : kind === 'mix' ? view.mixes : view.mics;
  }

  /** Saving a profile saves its pieces too. */
  function toSave(view: ProfilesView, kind: ProfileKind): boolean {
    return kind === 'profile' ? view.unsaved : view.dirty[kind];
  }

  function named(text: string, name: string): string {
    return text.replace('{name}', name);
  }

  async function run(kind: ProfileKind, command: ProfileCommand) {
    const error = await runProfileCommand(command);
    failed = error ? { kind, error } : null;
    if (!error) pending = null;
  }

  async function ask(next: Pending, suggestion = '') {
    pending = next;
    draft = suggestion;
    failed = null;
    await tick();
    field?.focus();
    field?.select();
  }

  /** Going to another profile or piece drops what is not saved: ask first. */
  function choose(view: ProfilesView, kind: ProfileKind, name: string) {
    if (toSave(view, kind)) void ask({ kind, action: 'select', name });
    else if (name !== view.active[kind]) void run(kind, { type: 'select', kind, name });
  }

  function confirm() {
    if (!pending) return;
    const { kind } = pending;
    if (pending.action === 'saveAs') {
      void run(kind, { type: 'saveAs', kind, name: draft });
    } else if (pending.action === 'rename' || pending.action === 'duplicate') {
      void run(kind, { type: pending.action, kind, name: pending.name, to: draft });
    } else {
      void run(kind, { type: pending.action, kind, name: pending.name });
    }
  }

  function cancel() {
    pending = null;
    failed = null;
  }
</script>

<section class="page">
  <h1>{i18n.t.nav.profiles}</h1>
  {#if profiles}
    <p class="hint">{i18n.t.profiles.hint}</p>
    {#each PROFILE_KINDS as kind (kind)}
      {@const text = i18n.t.profiles.kinds[kind]}
      {@const active = profiles.active[kind]}
      <section class="card" aria-label={text.title}>
        <div class="head">
          <div>
            <h2>{text.title}</h2>
            <p class="hint">{text.hint}</p>
          </div>
          <div class="actions">
            <button
              type="button"
              disabled={!toSave(profiles, kind)}
              onclick={() => run(kind, { type: 'save', kind })}
            >
              {i18n.t.profiles.save}
            </button>
            <button type="button" onclick={() => ask({ kind, action: 'saveAs' })}>
              {i18n.t.profiles.saveAs}
            </button>
          </div>
        </div>

        <ul role="radiogroup" aria-label={text.title}>
          {#each names(profiles, kind) as name (name)}
            <li>
              <!-- A button, not an input: it shows what is in use, never the click. -->
              <button
                type="button"
                class="name"
                role="radio"
                aria-checked={name === active}
                onclick={() => choose(profiles, kind, name)}
              >
                <span class="dot" aria-hidden="true"></span>
                <span class="text">{name}</span>
                {#if name === active}
                  <span class="label state">
                    {toSave(profiles, kind) ? i18n.t.profiles.modified : i18n.t.profiles.inUse}
                  </span>
                {/if}
              </button>
              <button
                type="button"
                class="small"
                aria-label={named(i18n.t.profiles.renameNamed, name)}
                onclick={() => ask({ kind, action: 'rename', name }, name)}
              >
                {i18n.t.profiles.rename}
              </button>
              <button
                type="button"
                class="small"
                aria-label={named(i18n.t.profiles.duplicateNamed, name)}
                onclick={() => ask({ kind, action: 'duplicate', name }, name)}
              >
                {i18n.t.profiles.duplicate}
              </button>
              <button
                type="button"
                class="small"
                aria-label={named(i18n.t.profiles.deleteNamed, name)}
                disabled={name === active}
                onclick={() => ask({ kind, action: 'delete', name })}
              >
                {i18n.t.profiles.delete}
              </button>
            </li>
          {/each}
        </ul>

        {#if pending?.kind === kind}
          <form
            class="ask"
            onsubmit={(event) => {
              event.preventDefault();
              confirm();
            }}
          >
            {#if pending.action === 'delete'}
              <p>{named(i18n.t.profiles.deleteQuestion, pending.name)}</p>
            {:else if pending.action === 'select'}
              <p>{named(i18n.t.profiles.switchQuestion, pending.name)}</p>
            {:else}
              <label>
                <span class="label">{i18n.t.profiles.name}</span>
                <input type="text" maxlength="60" bind:this={field} bind:value={draft} />
              </label>
            {/if}
            <button type="submit" class="main">
              {pending.action === 'delete'
                ? i18n.t.profiles.delete
                : pending.action === 'select'
                  ? i18n.t.profiles.switchAnyway
                  : i18n.t.profiles.confirm}
            </button>
            <button type="button" onclick={cancel}>{i18n.t.profiles.cancel}</button>
          </form>
        {/if}
        {#if failed?.kind === kind}
          <p class="error" role="alert">{i18n.t.profiles.errors[failed.error]}</p>
        {/if}
      </section>
    {/each}
    <p class="hint">{i18n.t.profiles.later}</p>
  {:else}
    <p class="hint">{i18n.t.mixer.connecting}</p>
  {/if}
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 820px;
  }

  .hint {
    color: var(--legend);
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px 20px;
    background: var(--panel);
    border: 1px solid var(--unlit);
    border-radius: 8px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  .card .hint {
    font-size: 13px;
  }

  .actions {
    display: flex;
    flex: none;
    gap: 8px;
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: stretch;
    gap: 6px;
  }

  button {
    padding: 7px 14px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  button:hover:not(:disabled) {
    border-color: var(--legend);
  }

  button:disabled {
    color: var(--unlit);
    cursor: default;
  }

  .small {
    padding: 4px 10px;
    color: var(--legend);
    font-size: 13px;
  }

  .small:hover:not(:disabled) {
    color: var(--silkscreen);
  }

  .name {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 12px;
    min-width: 0;
    background: var(--case);
    text-align: left;
  }

  .text {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    flex: none;
    width: 10px;
    height: 10px;
    border: 2px solid var(--unlit);
    border-radius: 50%;
  }

  /* What is in use is said in words, outlined and heavier; the amber dot
     only reinforces it. */
  .name[aria-checked='true'] {
    border-color: var(--silkscreen);
    font-weight: 600;
  }

  .name[aria-checked='true'] .dot {
    border-color: var(--amber);
    background: var(--amber);
  }

  .state {
    flex: none;
    color: var(--silkscreen);
  }

  .ask {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 8px;
    padding-top: 14px;
    border-top: 1px solid var(--unlit);
  }

  .ask p {
    flex: 1 1 100%;
  }

  label {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 6px;
  }

  input {
    padding: 7px 10px;
    border: 1px solid var(--legend);
    border-radius: 4px;
    background: var(--case);
    color: inherit;
    font: inherit;
    user-select: text;
  }

  .main {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
    font-weight: 600;
  }

  .error {
    font-weight: 600;
  }
</style>
