import { describe, it, expect } from "vitest";
import { exportState, saveExportPrefs } from "./exportState.svelte.js";

describe("export vertical slice", () => {
  it("initial exportState defaults", () => {
    expect(exportState.edge).toBe(2048);
    expect(exportState.border).toBe(false);
    expect(exportState.folder).toBe("");
  });

  it("exportState can be mutated reactively", () => {
    exportState.edge = 4096;
    expect(exportState.edge).toBe(4096);
    exportState.border = true;
    expect(exportState.border).toBe(true);

    // reset
    exportState.edge = 2048;
    exportState.border = false;
  });

  it("coordinates exports through createExportController", async () => {
    const { createExportController } = await import("./exportController.js");
    const frames = [{ path: "/photos/img1.jpg" }, { path: "/photos/img2.jpg" }];
    const ctrl = createExportController({
      getView: () => frames,
      getSel: () => 0,
      getSelection: () => ({ paths: new Set(["/photos/img1.jpg"]) }),
      getPhotoPath: () => "/photos/img1.jpg",
      getRecipe: () => ({ exposure: 0 }),
      getPicked: () => "flagged",
      exportState,
      selectedFrames: (v) => v.slice(0, 1),
    });

    expect(typeof ctrl.exportGrid).toBe("function");
    expect(typeof ctrl.exportSelection).toBe("function");
    expect(typeof ctrl.exportCurrent).toBe("function");
    expect(typeof ctrl.exportToDailyNote).toBe("function");
    expect(typeof ctrl.exportSelectionToDailyNote).toBe("function");
    expect(typeof ctrl.exportTo).toBe("function");
  });
});
