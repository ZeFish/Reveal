import { describe, it, expect, vi } from "vitest";
import { dispatchShortcut, shouldIgnoreKeystroke } from "./shortcutDispatcher.js";

describe("shortcutDispatcher", () => {
  function makeEvent(key, modifiers = {}) {
    return {
      key,
      metaKey: !!modifiers.metaKey,
      ctrlKey: !!modifiers.ctrlKey,
      shiftKey: !!modifiers.shiftKey,
      defaultPrevented: false,
      preventDefault: vi.fn(),
      target: { tagName: "DIV" },
    };
  }

  function makeState(overrides = {}) {
    return {
      currentMode: "cull",
      fullscreen: false,
      spaceLook: false,
      previewFilter: false,
      devPanel: false,
      view: [{ path: "/photo1.jpg", name: "photo1.jpg" }, { path: "/photo2.jpg", name: "photo2.jpg" }],
      sel: 0,
      photoPath: null,
      recipe: null,
      lastEditedKey: null,
      developEngine: "spektra",
      developPhotoPercent: 90,
      cols: 4,
      marginScale: 1,
      zoneMaskPreview: null,
      dockedActiveZone: "global",
      selectionAnchor: null,
      ...overrides,
    };
  }

  it("ignores keystrokes in input elements", () => {
    const e = {
      key: " ",
      target: { tagName: "INPUT" },
      defaultPrevented: false,
      preventDefault: vi.fn(),
    };
    expect(shouldIgnoreKeystroke(/** @type {any} */ (e))).toBe(true);
  });

  it("handles Cmd-A to select all", () => {
    const e = makeEvent("a", { metaKey: true });
    const selectAll = vi.fn();
    const handled = dispatchShortcut(/** @type {any} */ (e), makeState(), { selectAll });
    expect(handled).toBe(true);
    expect(selectAll).toHaveBeenCalled();
    expect(e.preventDefault).toHaveBeenCalled();
  });

  it("handles photo zoom in Dev mode with + and -", () => {
    const setPhotoSize = vi.fn();
    const state = makeState({ currentMode: "dev", developPhotoPercent: 90 });

    const ePlus = makeEvent("+");
    dispatchShortcut(/** @type {any} */ (ePlus), state, { setPhotoSize });
    expect(setPhotoSize).toHaveBeenCalledWith(95);

    const eMinus = makeEvent("-");
    dispatchShortcut(/** @type {any} */ (eMinus), state, { setPhotoSize });
    expect(setPhotoSize).toHaveBeenCalledWith(85);
  });

  it("handles grid columns and margin in Cull mode with = and +", () => {
    const setCols = vi.fn();
    const setMarginScale = vi.fn();
    const state = makeState({ currentMode: "cull", cols: 4, marginScale: 1 });

    const eEqual = makeEvent("=");
    dispatchShortcut(/** @type {any} */ (eEqual), state, { setCols, setMarginScale });
    expect(setCols).toHaveBeenCalledWith(3);

    const ePlus = makeEvent("+");
    dispatchShortcut(/** @type {any} */ (ePlus), state, { setCols, setMarginScale });
    expect(setMarginScale).toHaveBeenCalledWith(1.25);
  });

  it("handles m for zone mask in Dev mode and layout toggle in Cull mode", () => {
    const setZoneMask = vi.fn();
    const toggleLayout = vi.fn();

    // Dev mode
    const devState = makeState({ currentMode: "dev", zoneMaskPreview: null, dockedActiveZone: "global" });
    dispatchShortcut(/** @type {any} */ (makeEvent("m")), devState, { setZoneMask, toggleLayout });
    expect(setZoneMask).toHaveBeenCalledWith("shadows");
    expect(toggleLayout).not.toHaveBeenCalled();

    // Cull mode
    const cullState = makeState({ currentMode: "cull" });
    dispatchShortcut(/** @type {any} */ (makeEvent("m")), cullState, { setZoneMask, toggleLayout });
    expect(toggleLayout).toHaveBeenCalled();
  });

  it("handles ratings 0..5", () => {
    const rate = vi.fn();
    const e = makeEvent("4");
    const handled = dispatchShortcut(/** @type {any} */ (e), makeState(), { rate });
    expect(handled).toBe(true);
    expect(rate).toHaveBeenCalledWith(4);
  });

  it("navigates via arrow keys", () => {
    const navigate = vi.fn();
    const e = makeEvent("ArrowRight");
    const handled = dispatchShortcut(/** @type {any} */ (e), makeState({ sel: 0 }), { navigate });
    expect(handled).toBe(true);
    expect(navigate).toHaveBeenCalledWith(1, { shiftKey: false, metaKey: false });
  });

  it("coordinates workflow and shortcuts through createKeyboardController", async () => {
    const { createKeyboardController } = await import("./keyboardController.js");
    const switchMode = vi.fn();
    const setSpaceLook = vi.fn();
    const rate = vi.fn();
    const focusAt = vi.fn();
    const selectOnly = vi.fn();

    const state = makeState({ sel: 0, view: [{ path: "/p1.jpg" }, { path: "/p2.jpg" }] });
    const ctrl = createKeyboardController({
      getState: () => state,
      actions: {
        switchMode,
        setSpaceLook,
        saveLayouts: vi.fn(),
        toggleSidebar: vi.fn(),
        exitFullscreen: vi.fn(),
        toggleFullscreen: vi.fn(),
        togglePreviewFilter: vi.fn(),
        toggleFocusMode: vi.fn(),
        toggleAppearance: vi.fn(),
        toggleLayout: vi.fn(),
        toggleStory: vi.fn(),
        toggleShortcuts: vi.fn(),
        toggleDevPanel: vi.fn(),
        cycleZoom: vi.fn(),
        selectAll: vi.fn(),
        copyImageToClipboard: vi.fn(),
        clearSelection: vi.fn(),
        undoRecipeEdit: vi.fn(),
        redoRecipeEdit: vi.fn(),
        exportSelection: vi.fn(),
        copySettings: vi.fn(),
        pasteSettings: vi.fn(),
        setZoneMask: vi.fn(),
        setPhotoSize: vi.fn(),
        setCols: vi.fn(),
        setMarginScale: vi.fn(),
        updateRecipe: vi.fn(),
        focusAt,
        selectRange: vi.fn(),
        selectOnly,
        prepareFullscreenFrame: vi.fn(),
        openPhoto: vi.fn(),
        rate,
        toggleSelected: vi.fn(),
        setAnchor: vi.fn(),
      },
    });

    const eRate = makeEvent("5");
    ctrl.onKey(/** @type {any} */ (eRate));
    expect(rate).toHaveBeenCalledWith(5);

    const handledSwitch = ctrl.applyWorkflowResult({ action: "SWITCH_MODE", to: "dev", spaceLook: true });
    expect(handledSwitch).toBe(true);
    expect(switchMode).toHaveBeenCalledWith("dev", { openDevPanel: true });
    expect(setSpaceLook).toHaveBeenCalledWith(true);
  });

  it("sends the before / after key to Develop, once per press", () => {
    const compareDown = vi.fn();
    for (const key of ["\\", "y", "Y"]) {
      const e = makeEvent(key);
      const handled = dispatchShortcut(/** @type {any} */ (e), makeState({ currentMode: "dev" }), { compareDown });
      expect(handled).toBe(true);
      expect(e.preventDefault).toHaveBeenCalled();
    }
    expect(compareDown).toHaveBeenCalledTimes(3);

    const repeat = { ...makeEvent("\\"), repeat: true };
    dispatchShortcut(/** @type {any} */ (repeat), makeState({ currentMode: "dev" }), { compareDown });
    expect(compareDown).toHaveBeenCalledTimes(3);
  });

  it("leaves the before / after key alone in the grid and with a modifier", () => {
    const compareDown = vi.fn();
    dispatchShortcut(/** @type {any} */ (makeEvent("y")), makeState({ currentMode: "cull" }), { compareDown });
    dispatchShortcut(/** @type {any} */ (makeEvent("y", { metaKey: true })), makeState({ currentMode: "dev" }), { compareDown });
    expect(compareDown).not.toHaveBeenCalled();
  });
});
