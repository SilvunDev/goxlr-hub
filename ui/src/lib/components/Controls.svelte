<script lang="ts">
  import { sendIntent } from '../backend';
  import { BLOCKS, emptyView, isButton, summary } from '../controls';
  import {
    DOUBLE_PRESS_MS,
    LONG_PRESS_MS,
    type ButtonId,
    type ButtonView,
    type Snapshot,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import ControlCell from './ControlCell.svelte';
  import GesturePanel from './GesturePanel.svelte';
  import MicSlider from './MicSlider.svelte';

  let { device }: { device: Snapshot | null } = $props();

  let selected = $state<ButtonId | null>(null);

  /** The count of the last press the screen knows of; nothing before the first state. */
  let seen: number | null | undefined;

  // Pressing a real button finds its case. A press that was already there when
  // the screen opened is not one.
  $effect(() => {
    if (!device) return;
    const press = device.lastPress ?? null;
    const count = press?.count ?? null;
    if (seen === undefined) {
      seen = count;
      return;
    }
    if (count === seen) return;
    seen = count;
    if (press && isButton(press.button)) selected = press.button;
  });

  const t = $derived(i18n.t.controls);
  const controls = $derived(device?.controls ?? null);
  /** The buttons are those of the virtual device: they can be pressed from here. */
  const demo = $derived(device?.device.kind === 'virtual');

  function viewOf(button: ButtonId): ButtonView {
    return controls?.buttons.find((view) => view.button === button) ?? emptyView(button);
  }

  function lit(button: ButtonId): boolean {
    const touched = device?.touched ?? device?.pressed ?? [];
    return touched.includes(button);
  }

  function setTimes(longPressMs: number, doublePressMs: number) {
    void sendIntent({ type: 'setPressTimes', longPressMs, doublePressMs });
  }
</script>

<section class="page">
  <h1>{i18n.t.nav.controls}</h1>
  {#if device && controls}
    <p class="hint">{t.hint}</p>
    <p class="hint">{demo ? t.findDemo : t.findHardware}</p>

    <div class="board">
      <div class="blocks">
        {#each BLOCKS as block (block.id)}
          <section class="block" aria-label={t.blocks[block.id]}>
            <span class="label">{t.blocks[block.id]}</span>
            <ul>
              {#each block.buttons as button (button)}
                {@const what = summary(viewOf(button), t, i18n.t.channels)}
                <li>
                  <ControlCell
                    name={t.buttons[button]}
                    summary={what}
                    label={t.cell.replace('{button}', t.buttons[button]).replace('{summary}', what)}
                    selected={selected === button}
                    lit={lit(button)}
                    pressable={demo}
                    onselect={() => (selected = button)}
                    onpress={(down) => sendIntent({ type: 'pressButton', button, down })}
                  />
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>

      <aside>
        {#if selected}
          <GesturePanel
            button={selected}
            name={t.buttons[selected]}
            view={viewOf(selected)}
            doublePressMs={controls.doublePressMs}
            longPressMs={controls.longPressMs}
          />
        {:else}
          <p class="hint choose">{t.choose}</p>
        {/if}

        <section class="timing" aria-label={t.timing}>
          <h2>{t.timing}</h2>
          <p class="hint">{t.timingHint}</p>
          <MicSlider
            name={t.longPress}
            min={LONG_PRESS_MS.min}
            max={LONG_PRESS_MS.max}
            value={controls.longPressMs}
            text={(ms) => t.ms.replace('{ms}', String(ms))}
            onset={(ms) => setTimes(ms, controls.doublePressMs)}
          />
          <MicSlider
            name={t.doublePress}
            min={DOUBLE_PRESS_MS.min}
            max={DOUBLE_PRESS_MS.max}
            value={controls.doublePressMs}
            text={(ms) => t.ms.replace('{ms}', String(ms))}
            onset={(ms) => setTimes(controls.longPressMs, ms)}
          />
        </section>

        <button type="button" class="reset" onclick={() => sendIntent({ type: 'resetControls', button: null })}>
          {t.resetAll}
        </button>
      </aside>
    </div>
  {:else}
    <p class="hint">{t.connecting}</p>
  {/if}
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .hint {
    margin: 0;
    max-width: 640px;
    color: var(--legend);
  }

  .board {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 40px;
    margin-top: 14px;
  }

  .blocks {
    display: flex;
    flex: 1 1 420px;
    flex-direction: column;
    gap: 22px;
    min-width: 0;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  ul {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  aside {
    display: flex;
    flex: 0 0 340px;
    flex-direction: column;
    gap: 22px;
    max-width: 100%;
  }

  .timing {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 18px;
    border-top: 1px solid var(--unlit);
  }

  .timing h2 {
    margin: 0;
    font-size: 16px;
  }

  .timing :global(.row) {
    grid-template-columns: 96px minmax(80px, 1fr) 64px;
    gap: 10px;
  }

  .reset {
    align-self: flex-start;
    padding: 6px 10px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    cursor: pointer;
  }

  .reset:hover {
    border-color: var(--legend);
  }

  .choose {
    padding: 18px;
    border: 1px dashed var(--unlit);
    border-radius: 8px;
  }
</style>
