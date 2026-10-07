/**
 * Before / after: one key (or one button) for two gestures.
 *
 * A short press switches between the developed picture and the photo as the camera shot it, and
 * it stays where you left it. Holding the key shows the photo as shot for as long as it is held,
 * and letting go brings back what was there before: the quick look at a before without leaving
 * the edit.
 *
 * `onChange(showingBefore)` is told every time what to show; the screen follows. Time and the
 * clock are injected so the two gestures can be tested without waiting.
 */

/** How long a press must last to be a hold rather than a tap. */
export const HOLD_MS = 250;

/**
 * @param {{
 *   onChange: (showingBefore: boolean) => void,
 *   holdMs?: number,
 *   schedule?: (fn: () => void, ms: number) => any,
 *   cancel?: (handle: any) => void,
 * }} o
 */
export function createBeforeAfter({
  onChange,
  holdMs = HOLD_MS,
  schedule = (fn, ms) => setTimeout(fn, ms),
  cancel = (handle) => clearTimeout(handle),
}) {
  let latched = false;
  let peeking = false;
  let down = false;
  /** @type {any} */
  let timer = null;

  const tell = () => onChange(latched || peeking);

  return {
    /** The key or the button goes down. A key that repeats while held is not a new press. */
    press() {
      if (down) return;
      down = true;
      timer = schedule(() => {
        timer = null;
        peeking = true;
        tell();
      }, holdMs);
    },

    /** The key or the button comes up. */
    release() {
      if (!down) return;
      down = false;
      if (timer !== null) {
        // Let go before the hold: a tap, so switch and stay.
        cancel(timer);
        timer = null;
        latched = !latched;
        tell();
      } else if (peeking) {
        peeking = false;
        tell();
      }
    },

    /** Back to the developed picture: another photo, or leaving Develop. */
    reset() {
      if (timer !== null) cancel(timer);
      timer = null;
      down = false;
      const was = latched || peeking;
      latched = false;
      peeking = false;
      if (was) tell();
    },
  };
}
