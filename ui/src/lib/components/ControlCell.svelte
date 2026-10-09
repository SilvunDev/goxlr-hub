<script lang="ts">
  let {
    name,
    summary,
    label,
    selected,
    lit,
    pressable,
    onselect,
    onpress,
  }: {
    name: string;
    /** What the button does, in a few words. */
    summary: string;
    /** What a screen reader says of the case: its name and what it does. */
    label: string;
    selected: boolean;
    /** Held down, or let go a moment ago. */
    lit: boolean;
    /** The cell stands for a button of the virtual device: it can be pressed. */
    pressable: boolean;
    onselect: () => void;
    /** Presses or releases the button of the virtual device. */
    onpress: (down: boolean) => void;
  } = $props();

  /** The button is held down by this cell. */
  let down = false;
  /** A pointer press came before the click that follows it. */
  let pointed = false;

  function press() {
    if (down) return;
    down = true;
    onselect();
    onpress(true);
  }

  function release() {
    if (!down) return;
    down = false;
    onpress(false);
  }

  function onpointerdown(event: PointerEvent & { currentTarget: HTMLButtonElement }) {
    if (!pressable || event.button !== 0) return;
    pointed = true;
    try {
      event.currentTarget.setPointerCapture?.(event.pointerId);
    } catch {
      // Without capture, leaving the cell lets go of the button.
    }
    press();
  }

  function onclick() {
    if (!pressable) {
      onselect();
      return;
    }
    // A click that no pointer press came before: a screen reader or a script
    // asking for a press.
    if (pointed) {
      pointed = false;
      return;
    }
    press();
    release();
  }

  function onkeydown(event: KeyboardEvent) {
    if (!pressable || (event.key !== 'Enter' && event.key !== ' ')) return;
    // The key is the press: the click it would make must not come on top.
    event.preventDefault();
    if (!event.repeat) press();
  }

  function onkeyup(event: KeyboardEvent) {
    if (!pressable || (event.key !== 'Enter' && event.key !== ' ')) return;
    event.preventDefault();
    release();
  }

  // A cell that goes away while its button is held lets it go.
  $effect(() => () => release());
</script>

<button
  type="button"
  class="cell"
  class:selected
  class:lit
  aria-pressed={selected}
  aria-label={label}
  {onclick}
  {onpointerdown}
  onpointerup={release}
  onpointercancel={release}
  onpointerleave={release}
  {onkeydown}
  {onkeyup}
>
  <span class="name" aria-hidden="true">{name}</span>
  <span class="summary" aria-hidden="true">{summary}</span>
</button>

<style>
  .cell {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    min-height: 58px;
    padding: 8px 10px;
    border: 1px solid var(--unlit);
    border-radius: 6px;
    background: none;
    color: var(--legend);
    font-size: 12px;
    line-height: 1.25;
    text-align: left;
    cursor: pointer;
    touch-action: none;
  }

  .cell:hover {
    border-color: var(--legend);
  }

  .name {
    color: var(--silkscreen);
    font-size: 13px;
    font-weight: 600;
  }

  /* The chosen case has a bar on its edge and a thicker line: never colour alone. */
  .selected {
    border-color: var(--amber);
    border-left-width: 5px;
    padding-left: 6px;
  }

  /* A button that is held, or was a moment ago, is filled. */
  .lit,
  .lit:hover {
    background: var(--silkscreen);
    border-color: var(--silkscreen);
    color: var(--case);
  }

  .lit .name {
    color: var(--case);
  }

  .lit.selected {
    border-color: var(--amber);
  }
</style>
