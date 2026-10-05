import { describe, it, expect, beforeEach } from "vitest";
import {
  positionIn,
  anchorIn,
  focusAt,
  selectOnly,
  selectRange,
  toggle,
  clearSelection,
  setAnchor,
  selectedFrames,
} from "./selection.svelte.js";

/** @param {string[]} names */
const frames = (names) => names.map((n) => ({ path: `/nas/${n}.RAF`, name: n }));

const ASC = frames(["a", "b", "c", "d", "e"]);
const DESC = [...ASC].reverse();

describe("selection", () => {
  // The module is a singleton, so state leaks between tests unless it is
  // reset — which the first run of this suite proved by letting an anchor
  // from one test decide the answer in another.
  beforeEach(() => {
    clearSelection();
    setAnchor(null);
    focusAt([], 0); // drops the focus path and resets the fallback
  });

  /**
   * The bug this pins, in full: session restore looked the saved photo up in
   * `frames` and assigned the result to `sel`, which indexes `view`. With the
   * sort descending, that reopened the MIRROR photo — measured on a 61-photo
   * folder, positions 16 and 46, alternating every launch. Flipping the sort
   * did the same thing to the live selection.
   *
   * A position is only meaningful beside its own list. Here the list changes
   * under the focus and the focus must still name the same photo.
   */
  it("follows the photo when the list reverses", () => {
    focusAt(ASC, 1); // "b"
    expect(positionIn(ASC)).toBe(1);
    expect(positionIn(DESC)).toBe(3); // same photo, mirrored position
    expect(DESC[positionIn(DESC)].name).toBe("b");
  });

  it("follows the photo when the list is filtered", () => {
    focusAt(ASC, 3); // "d"
    const filtered = frames(["b", "d"]);
    expect(filtered[positionIn(filtered)].name).toBe("d");
  });

  /**
   * When the focused photo is gone — filtered out by a rating, hidden by
   * Editorial, moved, deleted — fall back the way `restorePhotoSelection`
   * always did: the first photo still selected, then the last position
   * clamped. Never silently to zero, which would throw the grid to the top.
   */
  it("falls back to a still-selected photo when the focus is gone", () => {
    focusAt(ASC, 4); // "e"
    selectOnly(ASC, 1); // selecting "b" also anchors it
    focusAt(ASC, 4);
    const without_e = frames(["a", "b", "c"]);
    expect(without_e[positionIn(without_e)].name).toBe("b");
  });

  it("falls back to the clamped last position when nothing is selected", () => {
    focusAt(ASC, 4);
    clearSelection();
    const shorter = frames(["a", "b"]);
    expect(positionIn(shorter)).toBe(1); // 4 clamped to the end, not 0
  });

  it("answers 0 for an empty list rather than -1", () => {
    focusAt(ASC, 2);
    expect(positionIn([])).toBe(0);
  });

  it("anchors a range on a photo, so the range survives a reversal", () => {
    // The focus must sit on a DIFFERENT photo, or falling back to it would
    // give the same answer and this would pass without testing anything —
    // which is exactly what the first version of this test did.
    selectOnly(ASC, 1); // anchors "b"
    focusAt(ASC, 3); // focus moves to "d"
    expect(ASC[anchorIn(ASC)].name).toBe("b");
    expect(ASC[positionIn(ASC)].name).toBe("d");
    expect(DESC[anchorIn(DESC)].name).toBe("b");
  });

  it("falls back to the focus when there is no anchor", () => {
    focusAt(ASC, 2);
    expect(anchorIn(ASC)).toBe(positionIn(ASC));
  });

  it("selects an inclusive range in either direction", () => {
    selectRange(ASC, 3, 1);
    expect(selectedFrames(ASC).map((f) => f.name)).toEqual(["b", "c", "d"]);
  });

  it("keeps what was selected when the range is additive", () => {
    selectOnly(ASC, 0);
    selectRange(ASC, 3, 4, true);
    expect(selectedFrames(ASC).map((f) => f.name)).toEqual(["a", "d", "e"]);
  });

  it("toggles one photo without disturbing the rest", () => {
    selectRange(ASC, 0, 2);
    toggle(ASC[1].path);
    expect(selectedFrames(ASC).map((f) => f.name)).toEqual(["a", "c"]);
    toggle(ASC[1].path);
    expect(selectedFrames(ASC).map((f) => f.name)).toEqual(["a", "b", "c"]);
  });

  it("acts on the focused photo when nothing is selected", () => {
    focusAt(ASC, 2);
    expect(selectedFrames(ASC).map((f) => f.name)).toEqual(["c"]);
  });

  it("ignores selected photos that are not in this list", () => {
    selectRange(ASC, 0, 4);
    const filtered = frames(["b", "d"]);
    expect(selectedFrames(filtered).map((f) => f.name)).toEqual(["b", "d"]);
  });
});
