<script lang="ts">
  import { sendIntent } from '../backend';
  import { BLOCKS, emptyView, isButton, namesOf, summary, wheelText } from '../controls';
  import {
    CHANNELS,
    DOUBLE_PRESS_MS,
    LONG_PRESS_MS,
    WHEELS,
    profilesOf,
    volumePercent,
    type ButtonId,
    type ButtonView,
    type ChannelId,
    type Snapshot,
    type WheelId,
    type WheelView,
  } from '../device';
  import { i18n } from '../i18n/index.svelte';
  import ControlCell from './ControlCell.svelte';
  import GesturePanel from './GesturePanel.svelte';
  import MicSlider from './MicSlider.svelte';
  import WheelPanel from './WheelPanel.svelte';

  let {
    device,
    focus = null,
  }: {
    device: Snapshot | null;
    /** A button asked for from elsewhere, to open its case. */
    focus?: { button: ButtonId; seq: number } | null;
  } = $props();

  let selected = $state<ButtonId | null>(null);
  let selectedWheel = $state<WheelId | null>(null);

  function pick(button: ButtonId) {
    selected = button;
    selectedWheel = null;
  }

  function pickWheel(wheel: WheelId) {
    selectedWheel = wheel;
    selected = null;
  }

  // Coming from the Mixer: the case of the button that was clicked.
  $effect(() => {
    if (focus) pick(focus.button);
  });

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
    if (press && isButton(press.button)) pick(press.button);
  });

  const t = $derived(i18n.t.controls);
  const controls = $derived(device?.controls ?? null);
  const names = $derived(namesOf(i18n.t));
  const profiles = $derived(device ? profilesOf(device) : null);
  /** The buttons are those of the virtual device: they can be pressed from here. */
  const demo = $derived(device?.device.kind === 'virtual');

  function viewOf(button: ButtonId): ButtonView {
    return controls?.buttons.find((view) => view.button === button) ?? emptyView(button);
  }

  function wheelOf(wheel: WheelId): WheelView {
    return controls?.wheels?.find((view) => view.wheel === wheel) ?? { wheel, action: null };
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
            {#if block.id === 'faders' && Array.isArray(device.faders)}
              <p class="hint">{t.faderHint}</p>
              <ul class="tracks">
                {#each device.faders as view (view.fader)}
                  <li>
                    <span class="fader">{i18n.t.mixer.fader} {view.fader.toUpperCase()}</span>
                    <select
                      aria-label={i18n.t.mixer.source.replace('{fader}', view.fader.toUpperCase())}
                      value={view.channel}
                      onchange={(event) =>
                        sendIntent({
                          type: 'assignFader',
                          fader: view.fader,
                          channel: event.currentTarget.value as ChannelId,
                        })}
                    >
                      {#each CHANNELS as channel (channel)}
                        <option value={channel}>{i18n.t.channels[channel]}</option>
                      {/each}
                    </select>
                    <span class="volume">{volumePercent(view.volume)}%</span>
                  </li>
                {/each}
              </ul>
            {/if}
            <ul>
              {#each block.buttons as button (button)}
                {@const what = summary(viewOf(button), t, names)}
                <li>
                  <ControlCell
                    name={t.buttons[button]}
                    summary={what}
                    label={t.cell.replace('{button}', t.buttons[button]).replace('{summary}', what)}
                    selected={selected === button}
                    lit={lit(button)}
                    pressable={demo}
                    onselect={() => pick(button)}
                    onpress={(down) => sendIntent({ type: 'pressButton', button, down })}
                  />
                </li>
              {/each}
            </ul>
          </section>
          {#if block.id === 'mic'}
            <section class="block" aria-label={t.blocks.wheels}>
              <span class="label">{t.blocks.wheels}</span>
              {#if demo}
                <p class="hint">{t.wheel.demo}</p>
              {/if}
              <ul>
                {#each WHEELS as wheel (wheel)}
                  {@const what = wheelText(wheelOf(wheel), t, names)}
                  <li class="wheel">
                    <ControlCell
                      name={t.wheels[wheel]}
                      summary={what}
                      label={t.cell.replace('{button}', t.wheels[wheel]).replace('{summary}', what)}
                      selected={selectedWheel === wheel}
                      lit={false}
                      pressable={false}
                      onselect={() => pickWheel(wheel)}
                      onpress={() => {}}
                    />
                    {#if demo}
                      <div class="turn">
                        <button
                          type="button"
                          aria-label={t.wheel.turnDown.replace('{wheel}', t.wheels[wheel])}
                          onclick={() => sendIntent({ type: 'turnWheel', wheel, notches: -1 })}
                        >
                          −
                        </button>
                        <button
                          type="button"
                          aria-label={t.wheel.turnUp.replace('{wheel}', t.wheels[wheel])}
                          onclick={() => sendIntent({ type: 'turnWheel', wheel, notches: 1 })}
                        >
                          +
                        </button>
                      </div>
                    {/if}
                  </li>
                {/each}
              </ul>
            </section>
          {/if}
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
            {profiles}
          />
        {:else if selectedWheel}
          <WheelPanel
            wheel={selectedWheel}
            name={t.wheels[selectedWheel]}
            view={wheelOf(selectedWheel)}
            dial={device.dials?.find((seen) => seen.wheel === selectedWheel) ?? null}
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
    gap: 32px;
    margin-top: 14px;
  }

  .blocks {
    display: flex;
    flex: 1 1 340px;
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
    grid-template-columns: repeat(auto-fill, minmax(112px, 1fr));
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  aside {
    display: flex;
    flex: 0 0 330px;
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

  .tracks {
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  }

  .tracks li {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border: 1px solid var(--unlit);
    border-radius: 6px;
    color: var(--legend);
    font-size: 12px;
  }

  .fader {
    color: var(--silkscreen);
    font-size: 13px;
    font-weight: 600;
  }

  .tracks select {
    padding: 4px 6px;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: var(--case);
    color: var(--silkscreen);
    font: inherit;
  }

  .volume {
    font-family: var(--font-mono);
  }

  .wheel {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .turn {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }

  .turn button {
    padding: 2px 0;
    border: 1px solid var(--unlit);
    border-radius: 4px;
    background: none;
    color: var(--silkscreen);
    font: inherit;
    cursor: pointer;
  }

  .turn button:hover {
    border-color: var(--legend);
  }

  .choose {
    padding: 18px;
    border: 1px dashed var(--unlit);
    border-radius: 8px;
  }
</style>
