/**
 * Where a value sits on a slider's rail, and back.
 *
 * The engine's ranges are lopsided on purpose — saturation runs -1..0.5,
 * clarity -40..60 — because the useful travel is not the same on both sides
 * of neutral. Laid out linearly, that put each slider's default at a
 * different spot on its rail: a column of sliders all at zero looked like a
 * column of sliders set to different things (Francis, 2026-09-23).
 *
 * So a slider whose default lies strictly inside its range gets two slopes:
 * the left half of the rail covers min → default, the right half default →
 * max. Every default lands dead centre, and the ends are still the ends. A
 * slider whose default IS an end (an amount that starts at 0) keeps a plain
 * linear rail — there is no "neutral middle" to line up.
 *
 * Positions are 0..1.
 */

/**
 * @param {number} value
 * @param {number} min
 * @param {number} max
 * @param {number | undefined} neutral the engine default; undefined → linear
 * @returns {number}
 */
export function toPosition(value, min, max, neutral) {
  const v = Math.min(max, Math.max(min, value));
  if (neutral === undefined || !(neutral > min && neutral < max)) {
    return (v - min) / (max - min);
  }
  if (v <= neutral) return 0.5 * ((v - min) / (neutral - min));
  return 0.5 + 0.5 * ((v - neutral) / (max - neutral));
}

/**
 * The inverse, snapped to the control's step (counted from `min`, as a native
 * range input counts it) and landing exactly on the default at the centre,
 * so a drag through the middle can always stop on true neutral.
 * @param {number} position
 * @param {number} min
 * @param {number} max
 * @param {number | undefined} neutral
 * @param {number} [step]
 * @returns {number}
 */
export function fromPosition(position, min, max, neutral, step) {
  const p = Math.min(1, Math.max(0, position));
  let v;
  if (neutral === undefined || !(neutral > min && neutral < max)) {
    v = min + p * (max - min);
  } else if (p === 0.5) {
    return neutral;
  } else if (p < 0.5) {
    v = min + (p / 0.5) * (neutral - min);
  } else {
    v = neutral + ((p - 0.5) / 0.5) * (max - neutral);
  }
  if (step && step > 0) {
    v = min + Math.round((v - min) / step) * step;
    // Undo float drift from the multiply (0.1 * 3 = 0.30000000000000004).
    const decimals = Math.max(0, -Math.floor(Math.log10(step)) + 1);
    v = Number(v.toFixed(decimals));
  }
  return Math.min(max, Math.max(min, v));
}
