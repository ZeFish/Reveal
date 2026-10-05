/**
 * Where a floating palette window should sit next to the main window.
 *
 * Pure geometry, separated from the Tauri calls that measure the windows, so
 * the cases that are awkward to reach by hand — a main window too wide for
 * either side, a second monitor, several palettes cascading — can be checked
 * without moving a real window around.
 *
 * Everything here is in LOGICAL pixels: the caller divides Tauri's physical
 * measurements by the scale factor before handing them over. Mixing the two
 * is the classic way to put a panel half off a retina screen.
 */

/** Gap between the main window and a palette. */
export const GAP = 12;
/** Each further palette steps down and right by this much. */
export const CASCADE = 34;

/**
 * @typedef {{ x: number, width: number }} Span A horizontal extent.
 * @typedef {{ x: number, y: number }} Point
 */

/**
 * @param {object} p
 * @param {{ x: number, y: number, width: number }} p.main The main window.
 * @param {Span} p.monitor The monitor the main window is on.
 * @param {Point[]} [p.otherMonitors] Any monitors that are not that one.
 * @param {number} p.index Which palette this is, 0-based.
 * @param {number} p.panelWidth
 * @returns {Point}
 */
export function placePalette({ main, monitor, otherMonitors = [], index, panelWidth }) {
  const step = index * CASCADE;

  // Right of the main window is the preferred side: the panel reads as an
  // extension of the photo rather than something covering the folder tree.
  const rightX = Math.round(main.x + main.width) + GAP;
  if (rightX + panelWidth <= monitor.x + monitor.width) {
    return { x: rightX + step, y: Math.round(main.y) + step };
  }

  const leftX = Math.round(main.x) - GAP - panelWidth;
  if (leftX >= monitor.x) {
    return { x: leftX + step, y: Math.round(main.y) + step };
  }

  // Neither side fits. A second screen is a better answer than overlapping
  // the photo.
  const other = otherMonitors[0];
  if (other) {
    return { x: Math.round(other.x + GAP) + step, y: Math.round(other.y + GAP) + step };
  }

  // One monitor, and a main window too wide for either side. Some overlap is
  // unavoidable; pin it to the right edge so at least it is predictable.
  return {
    x: Math.round(monitor.x + monitor.width - panelWidth) + step,
    y: Math.round(main.y) + step,
  };
}
