import { describe, it, expect, beforeEach, vi } from "vitest";
import { modalState } from "./modalState.svelte.js";
import {
  loadCatalogNote,
  saveCatalogNote,
  createCatalogDebouncer,
  initWhatsNew,
} from "./modalOperations.js";

describe("modules/modals/modalState", () => {
  beforeEach(() => {
    modalState.reset();
  });

  it("handles shortcuts visibility", () => {
    expect(modalState.shortcutsOpen).toBe(false);
    modalState.openShortcuts();
    expect(modalState.shortcutsOpen).toBe(true);
    modalState.closeShortcuts();
    expect(modalState.shortcutsOpen).toBe(false);
    modalState.toggleShortcuts();
    expect(modalState.shortcutsOpen).toBe(true);
    modalState.toggleShortcuts();
    expect(modalState.shortcutsOpen).toBe(false);
  });

  it("handles whatsNew release notes", () => {
    expect(modalState.whatsNew).toBe(null);
    modalState.setWhatsNew({ version: "0.9.0", notes: "Exciting new features!" });
    expect(modalState.whatsNew).toEqual({ version: "0.9.0", notes: "Exciting new features!" });
    modalState.closeWhatsNew();
    expect(modalState.whatsNew).toBe(null);
  });

  it("handles catalog note visibility and content", () => {
    expect(modalState.catalogOpen).toBe(false);
    expect(modalState.catalogContent).toBe("");

    modalState.openCatalog();
    expect(modalState.catalogOpen).toBe(true);

    modalState.setCatalogContent("# My Catalogue\nNotes here.");
    expect(modalState.catalogContent).toBe("# My Catalogue\nNotes here.");

    modalState.closeCatalog();
    expect(modalState.catalogOpen).toBe(false);
  });

  it("resets all modal state", () => {
    modalState.openShortcuts();
    modalState.setWhatsNew({ version: "1.0", notes: "notes" });
    modalState.openCatalog();
    modalState.setCatalogContent("sample");

    modalState.reset();
    expect(modalState.shortcutsOpen).toBe(false);
    expect(modalState.whatsNew).toBe(null);
    expect(modalState.catalogOpen).toBe(false);
    expect(modalState.catalogContent).toBe("");
  });
});

describe("modules/modals/modalOperations", () => {
  beforeEach(() => {
    modalState.reset();
    vi.restoreAllMocks();
  });

  it("loadCatalogNote fetches and populates catalogContent", async () => {
    const invoke = vi.fn().mockResolvedValue("# Notes");
    const result = await loadCatalogNote("/path/to/library", { invoke, state: modalState });

    expect(invoke).toHaveBeenCalledWith("load_catalog_note", { root: "/path/to/library" });
    expect(result).toBe("# Notes");
    expect(modalState.catalogContent).toBe("# Notes");
  });

  it("loadCatalogNote ignores empty root and handles invoke errors gracefully", async () => {
    const invoke = vi.fn().mockRejectedValue(new Error("Disk error"));
    const emptyResult = await loadCatalogNote("", { invoke, state: modalState });
    expect(emptyResult).toBe("");
    expect(invoke).not.toHaveBeenCalled();

    const errResult = await loadCatalogNote("/path/root", { invoke, state: modalState });
    expect(errResult).toBe("");
    expect(modalState.catalogContent).toBe("");
  });

  it("saveCatalogNote calls backend save with root and content", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await saveCatalogNote("/path/root", "Important note", { invoke });

    expect(invoke).toHaveBeenCalledWith("save_catalog_note", {
      root: "/path/root",
      content: "Important note",
    });
  });

  it("saveCatalogNote handles empty root safely", async () => {
    const invoke = vi.fn();
    await saveCatalogNote("", "Content", { invoke });
    expect(invoke).not.toHaveBeenCalled();
  });

  it("createCatalogDebouncer debounces multiple edits", async () => {
    vi.useFakeTimers();
    const saveFn = vi.fn();
    const debouncer = createCatalogDebouncer(saveFn, 300);

    debouncer("/root", "first");
    debouncer("/root", "second");
    debouncer("/root", "third");

    expect(saveFn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(299);
    expect(saveFn).not.toHaveBeenCalled();

    vi.advanceTimersByTime(2);
    expect(saveFn).toHaveBeenCalledTimes(1);
    expect(saveFn).toHaveBeenCalledWith("/root", "third");

    vi.useRealTimers();
  });

  it("initWhatsNew does nothing when isTauri is false", () => {
    const takeWhatsNew = vi.fn();
    const checkForUpdate = vi.fn();

    const cleanup = initWhatsNew({
      isTauri: false,
      takeWhatsNew,
      checkForUpdate,
      state: modalState,
    });

    expect(takeWhatsNew).not.toHaveBeenCalled();
    expect(checkForUpdate).not.toHaveBeenCalled();
    expect(modalState.whatsNew).toBe(null);
    cleanup();
  });

  it("initWhatsNew sets whatsNew data and schedules update check when isTauri is true", () => {
    vi.useFakeTimers();
    const takeWhatsNew = vi.fn().mockReturnValue({ version: "1.2.0", notes: "Changelog" });
    const checkForUpdate = vi.fn();

    const cleanup = initWhatsNew({
      isTauri: true,
      isDev: false,
      takeWhatsNew,
      checkForUpdate,
      state: modalState,
      checkDelayMs: 500,
    });

    expect(takeWhatsNew).toHaveBeenCalledTimes(1);
    expect(modalState.whatsNew).toEqual({ version: "1.2.0", notes: "Changelog" });
    expect(checkForUpdate).not.toHaveBeenCalled();

    vi.advanceTimersByTime(501);
    expect(checkForUpdate).toHaveBeenCalledWith({ quiet: true });

    cleanup();
    vi.useRealTimers();
  });

  it("initWhatsNew skips update check when isDev is true", () => {
    vi.useFakeTimers();
    const takeWhatsNew = vi.fn().mockReturnValue(null);
    const checkForUpdate = vi.fn();

    initWhatsNew({
      isTauri: true,
      isDev: true,
      takeWhatsNew,
      checkForUpdate,
      state: modalState,
      checkDelayMs: 500,
    });

    vi.advanceTimersByTime(1000);
    expect(checkForUpdate).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
