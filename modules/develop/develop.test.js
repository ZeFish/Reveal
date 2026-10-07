import { describe, it, expect, vi } from "vitest";
import { snapshotRecipe, MAX_RECIPE_UNDO_STEPS } from "./developState.svelte.js";
import {
  unpackFrame,
  withTimeout,
  recordRecipeCommit,
  undoRecipeEdit,
  redoRecipeEdit,
  setDevNum,
  resetOne,
  lutsKey,
  oldLutsKey,
  ensureLutMigration,
  addLutLayer,
  removeLutLayer,
  updateLutOpacity,
  setLutFile,
  applyEngineChange,
  toggleCheckLayer,
} from "./developOperations.js";

describe("developState & snapshotRecipe", () => {
  it("deep copies a recipe", () => {
    const original = { exposure_ev: 0.5, rapid_pre_luts: [{ name: "test.cube", opacity: 0.8 }] };
    const copy = snapshotRecipe(original);
    expect(copy).toEqual(original);
    expect(copy).not.toBe(original);
    expect(copy.rapid_pre_luts).not.toBe(original.rapid_pre_luts);
  });

  it("handles null recipe snapshot", () => {
    expect(snapshotRecipe(null)).toBeNull();
  });
});

describe("unpackFrame", () => {
  it("throws on empty or too small buffer", () => {
    expect(() => unpackFrame(null)).toThrow("empty frame buffer");
    expect(() => unpackFrame(new ArrayBuffer(8))).toThrow("frame buffer too small for header");
  });

  it("parses valid ArrayBuffer with 16-byte header and RGBA bytes", () => {
    const width = 2;
    const height = 2;
    const renderMs = 42;
    const pixelBytes = width * height * 4; // 16 bytes
    const totalBytes = 16 + pixelBytes; // 32 bytes

    const buffer = new ArrayBuffer(totalBytes);
    const view = new DataView(buffer);
    view.setUint32(0, width, true);
    view.setUint32(4, height, true);
    view.setUint32(8, renderMs, true);

    const u8 = new Uint8Array(buffer, 16);
    u8.fill(255);

    const frame = unpackFrame(buffer);
    expect(frame.width).toBe(2);
    expect(frame.height).toBe(2);
    expect(frame.renderMs).toBe(42);
    expect(frame.rgba.length).toBe(16);
    expect(frame.rgba[0]).toBe(255);
  });
});

describe("withTimeout", () => {
  it("resolves when promise settles before timeout", async () => {
    const promise = Promise.resolve("ok");
    const result = await withTimeout(promise, 500);
    expect(result).toBe("ok");
  });

  it("rejects when timeout triggers first", async () => {
    const slow = new Promise((resolve) => setTimeout(() => resolve("slow"), 100));
    await expect(withTimeout(slow, 10, "timed out!")).rejects.toThrow("timed out!");
  });
});

describe("undo & redo history", () => {
  it("manages undo and redo stacks correctly", () => {
    const state = {
      recipe: { exposure_ev: 0 },
      developEngine: "spektra",
      recipeUndoStack: [],
      recipeRedoStack: [],
      lastCommittedRecipe: { exposure_ev: 0 },
      restoringRecipeHistory: false,
    };

    // Commit 1: exposure 0 -> 1
    state.recipe = { exposure_ev: 1, engine: "spektra" };
    recordRecipeCommit(state);
    expect(state.recipeUndoStack.length).toBe(1);
    expect(state.recipeUndoStack[0].exposure_ev).toBe(0);

    // Commit 2: exposure 1 -> 2
    state.recipe = { exposure_ev: 2, engine: "spektra" };
    recordRecipeCommit(state);
    expect(state.recipeUndoStack.length).toBe(2);
    expect(state.recipeUndoStack[1].exposure_ev).toBe(1);

    // Undo 1
    const onEdited = vi.fn();
    undoRecipeEdit(state, "dev", onEdited);
    expect(state.recipe.exposure_ev).toBe(1);
    expect(state.recipeRedoStack.length).toBe(1);
    expect(state.recipeRedoStack[0].exposure_ev).toBe(2);
    expect(onEdited).toHaveBeenCalledWith(false);

    // Undo 2
    undoRecipeEdit(state, "dev", onEdited);
    expect(state.recipe.exposure_ev).toBe(0);
    expect(state.recipeRedoStack.length).toBe(2);

    // Redo 1
    redoRecipeEdit(state, "dev", onEdited);
    expect(state.recipe.exposure_ev).toBe(1);
    expect(state.recipeUndoStack.length).toBe(1);
    expect(state.recipeRedoStack.length).toBe(1);
  });

  it("respects MAX_RECIPE_UNDO_STEPS bounds", () => {
    const state = {
      recipe: { exposure_ev: 0 },
      developEngine: "spektra",
      recipeUndoStack: [],
      recipeRedoStack: [],
      lastCommittedRecipe: { exposure_ev: 0 },
    };

    for (let i = 1; i <= 60; i++) {
      state.recipe = { exposure_ev: i };
      recordRecipeCommit(state, 50);
    }
    expect(state.recipeUndoStack.length).toBe(50);
    expect(state.recipeUndoStack[0].exposure_ev).toBe(10);
  });
});

describe("setDevNum & resetOne", () => {
  it("sets scalar numbers and calls onEdited", () => {
    const state = { recipe: { exposure_ev: 0 }, lastEditedKey: "" };
    const onEdited = vi.fn();
    setDevNum(state, "exposure_ev", 1.5, false, undefined, onEdited);
    expect(state.recipe.exposure_ev).toBe(1.5);
    expect(state.lastEditedKey).toBe("exposure_ev");
    expect(onEdited).toHaveBeenCalledWith(false, "exposure_ev");
  });

  it("sets array element by index", () => {
    const state = { recipe: { tone_curve: [0, 0] }, lastEditedKey: "" };
    setDevNum(state, "tone_curve", 0.75, true, 1);
    expect(state.recipe.tone_curve[1]).toBe(0.75);
  });

  it("resets key using developDefaults", () => {
    const state = {
      recipe: { exposure_ev: 2 },
      developDefaults: { exposure_ev: 0 },
      lastEditedKey: "",
    };
    resetOne(state, "exposure_ev");
    expect(state.recipe.exposure_ev).toBe(0);
  });
});

describe("LUT operations", () => {
  it("provides correct stage keys", () => {
    expect(lutsKey("pre")).toBe("rapid_pre_luts");
    expect(lutsKey("post")).toBe("rapid_post_luts");
    expect(oldLutsKey("pre")).toBe("pre_luts");
    expect(oldLutsKey("post")).toBe("post_luts");
  });

  it("migrates legacy pre_luts to rapid_pre_luts", () => {
    const state = {
      recipe: { pre_luts: [{ name: "legacy.cube", opacity: 1 }] },
      developEngine: "rapid",
    };
    ensureLutMigration(state, "pre");
    expect(state.recipe.rapid_pre_luts).toEqual([{ name: "legacy.cube", opacity: 1 }]);
    expect(state.recipe.pre_luts).toEqual([]);
  });

  it("adds, updates opacity, changes file, and removes LUT layer", () => {
    const state = {
      recipe: {},
      developEngine: "rapid",
      luts: [{ name: "preset1.cube" }, { name: "preset2.cube" }],
    };

    addLutLayer(state, "pre");
    expect(state.recipe.rapid_pre_luts.length).toBe(1);
    expect(state.recipe.rapid_pre_luts[0].name).toBe("preset1.cube");

    updateLutOpacity(state, "pre", 0, 0.5);
    expect(state.recipe.rapid_pre_luts[0].opacity).toBe(0.5);

    setLutFile(state, "pre", 0, "preset2.cube");
    expect(state.recipe.rapid_pre_luts[0].name).toBe("preset2.cube");

    removeLutLayer(state, "pre", 0);
    expect(state.recipe.rapid_pre_luts.length).toBe(0);
  });
});

describe("checkLayer & engine changes", () => {
  it("cycles checkLayer mode none -> clipping -> none", () => {
    const state = { checkLayer: "none", lastActiveCheckLayer: "clipping" };
    toggleCheckLayer(state);
    expect(state.checkLayer).toBe("clipping");
    toggleCheckLayer(state);
    expect(state.checkLayer).toBe("none");
  });

  it("toggles explicit checkLayer mode", () => {
    const state = { checkLayer: "none", lastActiveCheckLayer: "clipping" };
    toggleCheckLayer(state, "zebra");
    expect(state.checkLayer).toBe("zebra");
    expect(state.lastActiveCheckLayer).toBe("zebra");
    toggleCheckLayer(state, "zebra");
    expect(state.checkLayer).toBe("none");
  });

  it("switches engine and calls hooks", () => {
    const state = { developEngine: null, recipe: {} };
    const onClear = vi.fn();
    const onEdited = vi.fn();

    applyEngineChange(state, "rapid", onClear, onEdited);
    expect(state.developEngine).toBe("rapid");
    expect(state.recipe.engine).toBe("rapid");
    expect(onEdited).toHaveBeenCalledWith(false);

    applyEngineChange(state, null, onClear, onEdited);
    expect(onClear).toHaveBeenCalled();
  });
});

describe("applyRecipeToFrames", () => {
  it("renders the photo on screen first, and saves the others without waiting one by one", async () => {
    const { applyRecipeToFrames } = await import("./developOperations.js");
    /** @type {string[]} */
    const calls = [];
    const state = { recipe: { engine: "spektra", crop_aspect: "1:1", crop_x: 0.1 }, developEngine: "spektra", useCanvas: false };
    const invoke = vi.fn(async (/** @type {string} */ cmd, /** @type {any} */ args) => {
      calls.push(`${cmd}:${args?.path ?? ""}`);
      return cmd === "load_sidecar" ? { engine_settings: {} } : undefined;
    });
    const scheduleRender = vi.fn(() => calls.push("render-on-screen"));
    await applyRecipeToFrames({
      state,
      recipeToApply: { engine: "rapid", exposure: 1 },
      targetFrames: [{ path: "/a.raw", name: "a" }, { path: "/b.raw", name: "b" }],
      photoPath: "/a.raw",
      invoke,
      scheduleRender,
      sendDevStateToPanel: () => {},
      PREVIEW_PX: 2048,
    });
    // Before any disk or NAS call, the open photo already has its new settings and its render.
    expect(calls[0]).toBe("render-on-screen");
    expect(state.recipe).toMatchObject({ engine: "rapid", exposure: 1, crop_aspect: "1:1", crop_x: 0.1 });
    // The open photo is rendered by the pump, not twice; the other one is developed and saved.
    expect(calls).not.toContain("develop_preview:/a.raw");
    expect(calls).toContain("develop_preview:/b.raw");
    expect(calls).toContain("save_recipe:/a.raw");
    expect(calls).toContain("save_recipe:/b.raw");
  });
});
