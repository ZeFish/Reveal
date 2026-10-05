import { describe, it, expect, vi } from "vitest";
import { createCatalogController } from "./catalogController.js";

describe("catalogController", () => {
  it("loads catalog note when root is present", async () => {
    const invoke = vi.fn().mockResolvedValue("# My Catalog");
    const modalState = { setCatalogContent: vi.fn() };

    const ctrl = createCatalogController({
      invoke,
      modalState: /** @type {any} */ (modalState),
      getRoot: () => "/photos",
    });

    await ctrl.loadCatalog();
    expect(invoke).toHaveBeenCalledWith("load_catalog_note", { root: "/photos" });
    expect(modalState.setCatalogContent).toHaveBeenCalledWith("# My Catalog");
  });

  it("updates state and triggers debounced save on catalogEdited", async () => {
    vi.useFakeTimers();
    const invoke = vi.fn().mockResolvedValue(null);
    const modalState = { setCatalogContent: vi.fn() };

    const ctrl = createCatalogController({
      invoke,
      modalState: /** @type {any} */ (modalState),
      getRoot: () => "/photos",
      delayMs: 100,
    });

    ctrl.catalogEdited("New note");
    expect(modalState.setCatalogContent).toHaveBeenCalledWith("New note");
    expect(invoke).not.toHaveBeenCalled();

    vi.advanceTimersByTime(150);
    expect(invoke).toHaveBeenCalledWith("save_catalog_note", {
      root: "/photos",
      content: "New note",
    });
    vi.useRealTimers();
  });
});
