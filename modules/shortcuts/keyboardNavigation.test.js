import { describe, it, expect, vi, beforeEach } from "vitest";
import { createKeyboardController } from "./keyboardController.js";
import { positionIn, focusAt, selectOnly, selectRange, selection, clearSelection } from "../core/stores/selection.svelte.js";

/** Ten photos, as the app's `view`. */
const view = Array.from({ length: 10 }, (_, i) => ({ path: `/p${i}.raw` }));

/**
 * The controller wired as the app wires it: `getState` is a plain object read at the moment of the
 * call (as in appController), and the actions move the real selection store.
 */
function setup(mode) {
  const opened = [];
  const controller = createKeyboardController({
    getState: () => ({
      currentMode: mode,
      fullscreen: false,
      view,
      sel: positionIn(view),
      photoPath: null,
      recipe: null,
      cols: 4,
      selectionAnchor: 0,
    }),
    actions: {
      focusAt: (v, i) => focusAt(v, i),
      selectOnly: (v, i) => selectOnly(v, i),
      selectRange: (v, a, b) => selectRange(v, a, b),
      openPhoto: (p) => opened.push(p),
      prepareFullscreenFrame: () => {},
    },
  });
  const press = (key) =>
    controller.onKey(/** @type {any} */ ({ key, shiftKey: false, metaKey: false, ctrlKey: false, altKey: false, target: null, preventDefault() {}, stopPropagation() {} }));
  return { press, opened };
}

describe("arrow keys", () => {
  beforeEach(() => {
    clearSelection();
    focusAt(view, 0);
    selectOnly(view, 0);
  });

  it("two right arrows in Develop open the photo two along, not one", () => {
    const { press, opened } = setup("dev");
    press("ArrowRight");
    press("ArrowRight");
    expect(opened).toEqual(["/p1.raw", "/p2.raw"]);
  });

  it("right then left comes back to the photo it started on", () => {
    const { press, opened } = setup("dev");
    press("ArrowRight");
    press("ArrowRight");
    press("ArrowLeft");
    expect(opened).toEqual(["/p1.raw", "/p2.raw", "/p1.raw"]);
    expect(selection.focusPath).toBe("/p1.raw");
  });

  it("the selection follows the focus, not the photo the focus just left", () => {
    const { press } = setup("cull");
    press("ArrowRight");
    press("ArrowRight");
    expect(selection.focusPath).toBe("/p2.raw");
    expect([...selection.paths]).toEqual(["/p2.raw"]);
  });
});
