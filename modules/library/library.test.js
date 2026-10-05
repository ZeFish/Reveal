import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  library,
  libraryState,
  beginOpen,
  leaveFolder,
  setRoots,
  setDirs,
  setScanning,
  setIndexProgress,
} from "./libraryState.svelte.js";
import {
  indexRoot,
  listLibraries,
  removeLibrary,
  rescanLibrary,
  rescan,
  rescanDir,
  revealDir,
  onPhotoDragStart,
  movePhotos,
  reconcileAfterFileOp,
  renameDir,
  createFolder,
  moveDir,
  withPreviewVersions,
  freshPreviewVersion,
  refreshDirs,
  pickFolder,
  loadCatalogNote,
  saveCatalogNote,
} from "./libraryOperations.js";

describe("libraryState", () => {
  beforeEach(() => {
    leaveFolder();
    setRoots([]);
    setDirs([]);
    setScanning(false);
    setIndexProgress(null);
  });

  it("updates scanning and indexProgress reactively", () => {
    expect(library.scanning).toBe(false);
    expect(library.indexProgress).toBe(null);

    libraryState.scanning = true;
    expect(library.scanning).toBe(true);

    libraryState.indexProgress = { dirs: 5, frames: 120 };
    expect(library.indexProgress).toEqual({ dirs: 5, frames: 120 });
  });
});

describe("libraryOperations", () => {
  beforeEach(() => {
    leaveFolder();
    setRoots(["/nas/photos"]);
    setDirs([{ dir: "/nas/photos/2026", name: "2026" }]);
  });

  it("indexRoot picks folder and refreshes dirs", async () => {
    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "pick_folder") return "/nas/new_root";
      if (cmd === "scan_root") return true;
      return null;
    });
    const refreshDirsMock = vi.fn().mockResolvedValue(true);
    const openDirMock = vi.fn();
    const state = { scanning: false };

    await indexRoot({
      invoke,
      refreshDirs: refreshDirsMock,
      openDir: openDirMock,
      state,
      library: { dirs: [{ dir: "/nas/new_root" }] },
    });

    expect(invoke).toHaveBeenCalledWith("pick_folder");
    expect(invoke).toHaveBeenCalledWith("scan_root", { path: "/nas/new_root" });
    expect(refreshDirsMock).toHaveBeenCalled();
    expect(openDirMock).toHaveBeenCalledWith("/nas/new_root");
    expect(state.scanning).toBe(false);
  });

  it("listLibraries returns catalog roots or empty on error", async () => {
    const invokeSuccess = vi.fn().mockResolvedValue([{ path: "/nas/photos", frame_count: 50 }]);
    const roots = await listLibraries({ invoke: invokeSuccess });
    expect(roots).toEqual([{ path: "/nas/photos", frame_count: 50 }]);

    const invokeError = vi.fn().mockRejectedValue(new Error("fail"));
    const log = vi.fn();
    const empty = await listLibraries({ invoke: invokeError, log });
    expect(empty).toEqual([]);
    expect(log).toHaveBeenCalled();
  });

  it("removeLibrary forgets root and leaves folder if curDir matches", async () => {
    beginOpen({ curDir: "/nas/photos/2026" });
    expect(library.curDir).toBe("/nas/photos/2026");

    const invoke = vi.fn().mockResolvedValue(100);
    const notify = vi.fn();
    const refreshDirsMock = vi.fn().mockResolvedValue(true);

    await removeLibrary("/nas/photos", {
      invoke,
      notify,
      refreshDirs: refreshDirsMock,
      curDir: library.curDir,
    });

    expect(invoke).toHaveBeenCalledWith("remove_catalog_root", { path: "/nas/photos" });
    expect(library.curDir).toBe(null);
    expect(refreshDirsMock).toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith(expect.stringContaining("100 photos forgotten"), 4000);
  });

  it("rescan delegates to Apple Photos if active", async () => {
    const refreshApplePhotos = vi.fn().mockResolvedValue(undefined);
    await rescan({
      invoke: vi.fn(),
      refreshDirs: vi.fn(),
      isApplePhotosActive: true,
      refreshApplePhotos,
    });
    expect(refreshApplePhotos).toHaveBeenCalled();
  });

  it("rescan rescans every catalog root and reloads curDir", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const refreshDirsMock = vi.fn().mockResolvedValue(true);
    const openDir = vi.fn();
    const state = { scanning: false };

    await rescan({
      invoke,
      refreshDirs: refreshDirsMock,
      openDir,
      state,
      library: {
        roots: ["/nas/a", "/nas/b"],
        curDir: "/nas/a/trip",
        dirs: [],
      },
    });

    expect(invoke).toHaveBeenCalledWith("scan_root", { path: "/nas/a" });
    expect(invoke).toHaveBeenCalledWith("scan_root", { path: "/nas/b" });
    expect(refreshDirsMock).toHaveBeenCalled();
    expect(openDir).toHaveBeenCalledWith("/nas/a/trip", true, false, true);
    expect(state.scanning).toBe(false);
  });

  it("rescanDir reindexes a folder", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const refreshDirsMock = vi.fn().mockResolvedValue(true);
    const hold = vi.fn();
    const state = { scanning: false };

    await rescanDir("/nas/photos/sub", {
      invoke,
      refreshDirs: refreshDirsMock,
      hold,
      state,
      roots: ["/nas/photos"],
    });

    expect(invoke).toHaveBeenCalledWith("scan_folder", { path: "/nas/photos/sub" });
    expect(refreshDirsMock).toHaveBeenCalled();
    expect(hold).toHaveBeenCalledWith("Reindexing sub…");
    expect(hold).toHaveBeenCalledWith("Reindexed sub");
  });

  it("onPhotoDragStart blocks apple-photos and populates transfer data for normal photos", () => {
    const hold = vi.fn();
    const mockEvent = {
      preventDefault: vi.fn(),
      dataTransfer: {
        setData: vi.fn(),
        effectAllowed: "",
      },
    };

    onPhotoDragStart("apple-photos://uuid", /** @type {any} */ (mockEvent), { hold });
    expect(mockEvent.preventDefault).toHaveBeenCalled();
    expect(hold).toHaveBeenCalledWith(expect.stringContaining("Export Apple Photos"));

    onPhotoDragStart("/nas/photos/p1.jpg", /** @type {any} */ (mockEvent), {
      selectedPaths: new Set(["/nas/photos/p1.jpg", "/nas/photos/p2.jpg"]),
    });
    expect(mockEvent.dataTransfer.effectAllowed).toBe("move");
    expect(mockEvent.dataTransfer.setData).toHaveBeenCalledWith(
      "application/x-reveal-photos",
      JSON.stringify(["/nas/photos/p1.jpg", "/nas/photos/p2.jpg"]),
    );
  });

  it("movePhotos moves files and reconciles index", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const refreshDirsMock = vi.fn().mockResolvedValue(true);
    const setProgress = vi.fn();
    const startActivity = vi.fn().mockReturnValue("job-1");
    const updateActivity = vi.fn();
    const releaseActive = vi.fn();
    const clearSelection = vi.fn();

    await movePhotos(["/nas/src/a.jpg", "/nas/src/b.jpg"], "/nas/dest", {
      invoke,
      refreshDirs: refreshDirsMock,
      setProgress,
      startActivity,
      updateActivity,
      releaseActive,
      clearSelection,
    });

    expect(invoke).toHaveBeenCalledWith("move_photo", { path: "/nas/src/a.jpg", destDir: "/nas/dest" });
    expect(invoke).toHaveBeenCalledWith("move_photo", { path: "/nas/src/b.jpg", destDir: "/nas/dest" });
    expect(invoke).toHaveBeenCalledWith("scan_folder", { path: "/nas/dest" });
    expect(invoke).toHaveBeenCalledWith("scan_folder", { path: "/nas/src" });
    expect(refreshDirsMock).toHaveBeenCalled();
    expect(clearSelection).toHaveBeenCalled();
    expect(setProgress).toHaveBeenCalledWith(null);
  });

  it("createFolder creates a directory and notifies", async () => {
    const invoke = vi.fn().mockResolvedValue("/nas/photos/NewFolder");
    const notify = vi.fn();

    const result = await createFolder("/nas/photos", "NewFolder", { invoke, notify });
    expect(invoke).toHaveBeenCalledWith("create_dir", { parentDir: "/nas/photos", name: "NewFolder" });
    expect(notify).toHaveBeenCalledWith("Folder created: NewFolder", 2500);
    expect(result).toBe("/nas/photos/NewFolder");
  });

  it("renameDir renames directory and reconciles", async () => {
    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "rename_dir") return "/nas/photos/Renamed";
      return true;
    });
    const refreshDirsMock = vi.fn().mockResolvedValue(true);
    const openDir = vi.fn();
    const notify = vi.fn();

    const result = await renameDir("/nas/photos/Old", "Renamed", {
      invoke,
      notify,
      refreshDirs: refreshDirsMock,
      curDir: "/nas/photos/Old",
      openDir,
    });

    expect(invoke).toHaveBeenCalledWith("rename_dir", { path: "/nas/photos/Old", newName: "Renamed" });
    expect(openDir).toHaveBeenCalledWith("/nas/photos/Renamed");
    expect(notify).toHaveBeenCalledWith("Renamed → Renamed", 3000);
    expect(result).toBe("/nas/photos/Renamed");
  });

  it("withPreviewVersions stamps previewVersion on frames", async () => {
    const invoke = vi.fn().mockResolvedValue([12345, 67890]);
    const rows = [{ path: "/nas/a.jpg" }, { path: "/nas/b.jpg" }];

    const stamped = await withPreviewVersions(rows, { invoke, isTauri: true });
    expect(stamped[0].previewVersion).toBe(12345);
    expect(stamped[1].previewVersion).toBe(67890);
  });

  it("loadCatalogNote and saveCatalogNote interact with backend", async () => {
    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "load_catalog_note") return "# My Catalog";
      if (cmd === "save_catalog_note") return true;
      return null;
    });

    const note = await loadCatalogNote("/nas/root", { invoke });
    expect(note).toBe("# My Catalog");

    await saveCatalogNote("/nas/root", "# Updated Note", { invoke });
    expect(invoke).toHaveBeenCalledWith("save_catalog_note", {
      root: "/nas/root",
      content: "# Updated Note",
    });
  });
});
