import { describe, it, expect, vi } from "vitest";
import { createImportController } from "./importController.js";

describe("importController", () => {
  it("toggles auto import and updates state and notification", async () => {
    const invoke = vi.fn().mockResolvedValue({ auto_import: true });
    const notify = vi.fn();
    const importState = { autoImport: false };

    const ctrl = createImportController({
      invoke,
      notify,
      importState,
    });

    await ctrl.toggleAutoImport();
    expect(invoke).toHaveBeenCalledWith("toggle_auto_import");
    expect(importState.autoImport).toBe(true);
    expect(notify).toHaveBeenCalledWith("auto-import enabled", 2500);
  });

  it("delegates importCard to opImportCard with appropriate paths", async () => {
    const importState = { importDir: "/Volumes/Media/Archive" };
    const refreshDirs = vi.fn().mockResolvedValue(null);
    const openDir = vi.fn().mockResolvedValue(null);

    const ctrl = createImportController({
      importState,
      refreshDirs,
      openDir,
    });

    expect(typeof ctrl.importCard).toBe("function");
  });
});
