/** How long a value set on screen is shown before the device is believed again. */
const HOLD_MS = 300;

/**
 * A value being set on screen. It is shown instead of what the device
 * reports for a short while, so that a control does not jump back to a state
 * the device sent before it heard of the change.
 */
export function createHold() {
  let value = $state<number | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  return {
    get value() {
      return value;
    },
    set(next: number) {
      value = next;
      clearTimeout(timer);
      timer = setTimeout(() => (value = null), HOLD_MS);
    },
  };
}
