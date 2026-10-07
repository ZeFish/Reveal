/**
 * Keeping a crop frame inside a straightened picture.
 *
 * The straighten angle turns the picture about the centre of the frame; the crop rectangle
 * stays upright. Wherever the rectangle sticks out past the turned picture there is nothing
 * to show, so the Crop tab keeps it inside (Lightroom does the same): it slides the frame back
 * in, and shrinks it — keeping its proportions — only when it cannot fit at its size.
 *
 * Geometry. Work in the photo's own proportions, height 1 and width `aspect`. A frame with
 * centre C and half-sizes (a, b) lies inside the turned picture exactly when, in the picture's
 * own axes (C turned back by the angle), its centre keeps
 *     |x| ≤ A − (a·|cos| + b·|sin|)      and      |y| ≤ B − (a·|sin| + b·|cos|)
 * with (A, B) = (aspect/2, 1/2). So the legal centres form a rectangle: the fit is a clamp,
 * not a search, and the size that still fits is a ratio.
 *
 * @typedef {{ x: number, y: number, w: number, h: number }} Box  normalised to the photo, 0..1
 */

const EPS = 1e-9;

/** @param {number} deg */
const trig = (deg) => {
  const rad = (deg * Math.PI) / 180;
  return { c: Math.cos(rad), s: Math.sin(rad) };
};

/**
 * Is the whole frame inside the turned picture?
 * @param {Box} box
 * @param {number} angle degrees, clockwise positive (the straighten slider)
 * @param {number} aspect the photo's width / height
 */
export function insideTurned(box, angle, aspect) {
  const { c, s } = trig(angle);
  const a = (box.w * aspect) / 2;
  const b = box.h / 2;
  const cx = (box.x + box.w / 2 - 0.5) * aspect;
  const cy = box.y + box.h / 2 - 0.5;
  // The centre, turned back into the picture's own axes.
  const px = cx * c + cy * s;
  const py = -cx * s + cy * c;
  return (
    Math.abs(px) <= aspect / 2 - (a * Math.abs(c) + b * Math.abs(s)) + EPS &&
    Math.abs(py) <= 0.5 - (a * Math.abs(s) + b * Math.abs(c)) + EPS
  );
}

/**
 * The frame nearest to `box` that lies inside the turned picture: same proportions, the same
 * size when it fits (slid back in), smaller when it does not.
 * @param {Box} box
 * @param {number} angle
 * @param {number} aspect
 * @returns {Box}
 */
export function fitInsideTurned(box, angle, aspect) {
  if (!(aspect > 0) || !(box.w > 0) || !(box.h > 0)) return box;
  const { c, s } = trig(angle);
  const ac = Math.abs(c);
  const as = Math.abs(s);
  let a = (box.w * aspect) / 2;
  let b = box.h / 2;

  // Largest scale at which the frame fits at all.
  const k = Math.min(1, (aspect / 2) / (a * ac + b * as || 1), 0.5 / (a * as + b * ac || 1));
  a *= k;
  b *= k;
  const mx = Math.max(0, aspect / 2 - (a * ac + b * as));
  const my = Math.max(0, 0.5 - (a * as + b * ac));

  const cx = (box.x + box.w / 2 - 0.5) * aspect;
  const cy = box.y + box.h / 2 - 0.5;
  const px = Math.max(-mx, Math.min(mx, cx * c + cy * s));
  const py = Math.max(-my, Math.min(my, -cx * s + cy * c));
  // Back out of the picture's axes.
  const nx = px * c - py * s;
  const ny = px * s + py * c;

  const w = box.w * k;
  const h = box.h * k;
  return { x: nx / aspect + 0.5 - w / 2, y: ny + 0.5 - h / 2, w, h };
}
