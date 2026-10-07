import { describe, it, expect, vi } from "vitest";
import { createLibraryController } from "./libraryController.js";
import { library, beginOpen, leaveFolder } from "./libraryState.svelte.js";

describe("createLibraryController", () => {
  it("provides bound operations that delegate to library operations", async () => {
    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "catalog_roots") return [{ path: "/photos", frame_count: 42, mounted: true }];
      if (cmd === "remove_catalog_root") return 42;
      if (cmd === "reveal_dir") return true;
      if (cmd === "create_folder") return { ok: true };
      return null;
    });
    const notify = vi.fn();
    const hold = vi.fn();
    const refreshDirs = vi.fn().mockResolvedValue(true);
    const openDir = vi.fn();

    const controller = createLibraryController({
      invoke,
      notify,
      hold,
      refreshDirs,
      openDir,
      state: { scanning: false },
      library: { roots: ["/photos"], dirs: [], curDir: "/photos/sub" },
      getSelectionPaths: () => ["/photos/sub/1.jpg"],
    });

    const roots = await controller.listLibraries();
    expect(roots).toHaveLength(1);
    expect(roots[0].frame_count).toBe(42);

    await controller.removeLibrary("/photos");
    expect(invoke).toHaveBeenCalledWith("remove_catalog_root", { path: "/photos" });
    expect(refreshDirs).toHaveBeenCalled();

    await controller.revealDir("/photos");
    expect(invoke).toHaveBeenCalledWith("open_path", { path: "/photos" });

    await controller.createFolder("/photos", "NewAlbum");
    expect(invoke).toHaveBeenCalledWith("create_dir", { parentDir: "/photos", name: "NewAlbum" });
  });

  it("handles moveSelectedPhotosToDir fallback to currentPhotoPath", async () => {
    const invoke = vi.fn().mockResolvedValue([]);
    const notify = vi.fn();
    const refreshDirs = vi.fn().mockResolvedValue(true);
    const openDir = vi.fn();

    const controller = createLibraryController({
      invoke,
      notify,
      refreshDirs,
      openDir,
      state: { scanning: false },
      library: { roots: ["/photos"], dirs: [], curDir: "/photos" },
      getSelectionPaths: () => [],
      getCurrentPhotoPath: () => "/photos/fallback.jpg",
    });

    await controller.moveSelectedPhotosToDir("/photos/dest");
    expect(invoke).toHaveBeenCalledWith("move_photo", {
      path: "/photos/fallback.jpg",
      destDir: "/photos/dest",
    });
  });

  it("coordinates directory and folder opening via createNavigationController", async () => {
    const { createNavigationController } = await import("./navigationController.js");
    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "index_frames") return [{ path: "/photos/1.jpg", name: "1.jpg" }];
      if (cmd === "list_dir") return [{ path: "/folder/2.jpg", name: "2.jpg" }];
      return null;
    });

    const session = {
      setLastDirectory: vi.fn(),
      lastPhoto: vi.fn().mockReturnValue(null),
    };
    const cullingState = { minRating: 2, layout: "uniform" };
    const storyState = { filterStory: true };
    const commit = vi.fn().mockReturnValue(true);
    const replace = vi.fn();
    const finish = vi.fn();
    const beginOpen = vi.fn().mockReturnValue({ commit, replace, finish });
    const withPreviewVersions = vi.fn().mockImplementation(async (rows) => rows);
    const openFolderSession = vi.fn().mockReturnValue({ scroll: 150, mode: "cull" });
    const switchMode = vi.fn();
    const refreshStory = vi.fn();
    const focusAt = vi.fn();
    const selectOnly = vi.fn();
    const setFolderSession = vi.fn();
    const setCurrentScrollTop = vi.fn();
    const setPreviewFilter = vi.fn();
    const setLoading = vi.fn();

    const navCtrl = createNavigationController({
      invoke,
      session,
      cullingState,
      storyState,
      beginOpen,
      withPreviewVersions,
      openFolderSession,
      scrollOffsets: {},
      switchMode,
      refreshStory,
      focusAt,
      selectOnly,
      openPhoto: vi.fn(),
      layouts: { dev: {} },
      getView: () => [{ path: "/photos/1.jpg" }],
      leaveApplePhotos: vi.fn(),
      leaveImmich: vi.fn(),
      handleOpenApplePhotos: vi.fn(),
      handleOpenImmich: vi.fn(),
      setFolderSession,
      setCurrentScrollTop,
      setPreviewFilter,
      setLoading,
    });

    await navCtrl.openDir("/photos");
    expect(beginOpen).toHaveBeenCalledWith({ curDir: "/photos" });
    expect(session.setLastDirectory).toHaveBeenCalledWith("/photos");
    expect(cullingState.minRating).toBe(0);
    expect(storyState.filterStory).toBe(false);
    expect(setPreviewFilter).toHaveBeenCalledWith(false);
    expect(focusAt).toHaveBeenCalled();
    expect(selectOnly).toHaveBeenCalled();
    expect(setFolderSession).toHaveBeenCalled();
    expect(setCurrentScrollTop).toHaveBeenCalledWith(150);
    expect(switchMode).toHaveBeenCalledWith("cull");
    expect(refreshStory).toHaveBeenCalled();
    expect(setLoading).toHaveBeenCalledWith(false);

    await navCtrl.openFolder("/external/folder");
    expect(beginOpen).toHaveBeenCalledWith({ folder: "/external/folder" });
    expect(finish).toHaveBeenCalled();
  });

  // The import's progress handler and the `preview-published` handler both reach the photo list
  // through the controller. It once had no such method; the wiring called it with `?.()` and
  // nothing happened, so photos copied into the folder you were looking at never appeared.
  it("replaces the photos of the open folder, so an import shows up while it runs", () => {
    leaveFolder();
    beginOpen({ curDir: "/nas/Import" }).commit([{ path: "/nas/Import/a.RAF", name: "a.RAF" }]);
    const controller = createLibraryController({
      invoke: vi.fn(),
      refreshDirs: vi.fn(),
      openDir: vi.fn(),
      state: {},
      library,
    });
    controller.refreshLoadedFrames([
      { path: "/nas/Import/a.RAF", name: "a.RAF" },
      { path: "/nas/Import/b.RAF", name: "b.RAF" },
    ]);
    expect(library.frames.map((f) => f.name)).toEqual(["a.RAF", "b.RAF"]);
    expect(library.curDir).toBe("/nas/Import");
  });
});
