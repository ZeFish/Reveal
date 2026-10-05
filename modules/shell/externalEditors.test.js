import { describe, it, expect, vi } from "vitest";
import { createExternalEditorsController } from "./externalEditors.svelte.js";

describe("externalEditorsController", () => {
  it("initializes with empty installedEditors and populates on loadExternalEditors", async () => {
    const editorsList = [["Photoshop", "/Applications/Photoshop.app"]];
    const invoke = vi.fn().mockResolvedValue(editorsList);

    const ctrl = createExternalEditorsController({
      isTauri: true,
      invoke,
    });

    expect(ctrl.installedEditors).toEqual([]);
    await ctrl.loadExternalEditors();
    expect(invoke).toHaveBeenCalledWith("list_external_editors");
    expect(ctrl.installedEditors).toEqual(editorsList);
  });

  it("opens frame in selected external editor", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const notify = vi.fn();

    const ctrl = createExternalEditorsController({
      invoke,
      notify,
      getCurrentPhotoPath: () => "/photos/dsc001.raw",
    });

    await ctrl.openInEditor("/Applications/Affinity Photo.app");
    expect(invoke).toHaveBeenCalledWith("open_in_editor", {
      filePath: "/photos/dsc001.raw",
      appPath: "/Applications/Affinity Photo.app",
    });
    expect(notify).toHaveBeenCalledWith("Opened successfully ✓", 2000);
  });

  it("ignores openInEditor when path is missing", async () => {
    const invoke = vi.fn();
    const ctrl = createExternalEditorsController({
      invoke,
      getCurrentPhotoPath: () => null,
    });

    await ctrl.openInEditor("/Applications/Photoshop.app");
    expect(invoke).not.toHaveBeenCalled();
  });
});
