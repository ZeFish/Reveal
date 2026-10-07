/**
 * Dragging a point of a tone curve.
 *
 * The end points are the curve's black point and white point, and they move both ways: up and
 * down for the level, left and right for where the curve starts or stops. Beyond an end point
 * the curve holds that end's value (curves.rs::build_lut does the same), so moving the black
 * point to the right clips everything left of it to the black level, and moving the white point
 * to the left clips everything right of it to the white level. An end point cannot pass its
 * neighbour, and a point in the middle cannot pass an end point, so the first point stays the
 * first and the last stays the last.
 */

/** The closest two points may be, in curve space (0..1). */
export const MIN_GAP = 0.01;

/** @param {number} v @param {number} lo @param {number} hi */
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

/**
 * @param {number[][]} points control points, sorted by x
 * @param {number} index the point being dragged
 * @param {number} x where the pointer is, 0..1
 * @param {number} y where the pointer is, 0..1
 * @returns {{ points: number[][], index: number }} the points, sorted, and where the dragged one now is
 */
export function movePoint(points, index, x, y) {
  const next = points.map((p) => [...p]);
  const last = next.length - 1;
  let nx;
  if (index === 0) nx = clamp(x, 0, next[1][0] - MIN_GAP);
  else if (index === last) nx = clamp(x, next[last - 1][0] + MIN_GAP, 1);
  else nx = clamp(x, next[0][0] + MIN_GAP, next[last][0] - MIN_GAP);
  const moved = [nx, clamp(y, 0, 1)];
  next[index] = moved;
  next.sort((a, b) => a[0] - b[0]);
  return { points: next, index: next.indexOf(moved) };
}
