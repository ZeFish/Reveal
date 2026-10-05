import { describe, it, expect, vi, beforeEach } from "vitest";
import { cullingState, MASONRY_LIMIT, initCullingState } from "./cullingState.svelte.js";
import {
  saveGridPrefs,
  toggleLayout,
  rate,
  stopCull,
  handleCullStarted,
  handleCullProgress,
  handleCullFinished,
  handleCullFailed,
} from "./cullingOperations.js";
import { invoke } from "@tauri-apps/api/core";
import { activity } from "@modules/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("modules/culling", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cullingState.cols = 4;
    cullingState.marginScale = 1;
    cullingState.cellAspect = 1.5;
    cullingState.fillCells = true;
    cullingState.sortDesc = false;
    cullingState.layout = "uniform";
    cullingState.minRating = 0;
    cullingState.cullTaskId = null;
    cullingState.aiCullMarkStory = false;
    cullingState.aiCullExportDesktop = false;
    cullingState.aiCullTarget = 24;
  });

  describe("initCullingState", () => {
    it("initializes state from grid preferences and session", () => {
      initCullingState({
        gridPrefs: {
          cols: 6,
          marginScale: 1.2,
          cellAspect: 1.0,
          fillCells: false,
          sortDesc: true,
        },
        gridLayout: "masonry",
        preferences: {
          ai_cull_mark_story: true,
          ai_cull_export_desktop: true,
          ai_cull_target: 48,
        },
      });

      expect(cullingState.cols).toBe(6);
      expect(cullingState.marginScale).toBe(1.2);
      expect(cullingState.cellAspect).toBe(1.0);
      expect(cullingState.fillCells).toBe(false);
      expect(cullingState.sortDesc).toBe(true);
      expect(cullingState.layout).toBe("masonry");
      expect(cullingState.aiCullMarkStory).toBe(true);
      expect(cullingState.aiCullExportDesktop).toBe(true);
      expect(cullingState.aiCullTarget).toBe(48);
    });
  });

  describe("saveGridPrefs", () => {
    it("calls session.setGridPrefs with current culling state", () => {
      const mockSession = { setGridPrefs: vi.fn() };
      cullingState.cols = 5;
      cullingState.marginScale = 0.8;

      saveGridPrefs(mockSession);

      expect(mockSession.setGridPrefs).toHaveBeenCalledWith({
        cols: 5,
        marginScale: 0.8,
        cellAspect: 1.5,
        fillCells: true,
        sortDesc: false,
      });
    });
  });

  describe("toggleLayout", () => {
    it("blocks toggle to masonry if totalFrames exceeds MASONRY_LIMIT", () => {
      const mockSession = { setGridLayout: vi.fn() };
      cullingState.layout = "uniform";

      toggleLayout({ totalFrames: MASONRY_LIMIT + 1, session: mockSession });

      expect(cullingState.layout).toBe("uniform");
      expect(mockSession.setGridLayout).not.toHaveBeenCalled();
    });

    it("toggles to masonry if totalFrames <= MASONRY_LIMIT", () => {
      const mockSession = { setGridLayout: vi.fn() };
      cullingState.layout = "uniform";

      toggleLayout({ totalFrames: 100, session: mockSession });

      expect(cullingState.layout).toBe("masonry");
      expect(mockSession.setGridLayout).toHaveBeenCalledWith("masonry");
    });

    it("toggles back from masonry to uniform regardless of frame count", () => {
      const mockSession = { setGridLayout: vi.fn() };
      cullingState.layout = "masonry";

      toggleLayout({ totalFrames: 800, session: mockSession });

      expect(cullingState.layout).toBe("uniform");
      expect(mockSession.setGridLayout).toHaveBeenCalledWith("uniform");
    });
  });

  describe("rate", () => {
    it("does nothing when no frames are selected", async () => {
      await rate([], 4);
      expect(invoke).not.toHaveBeenCalled();
    });

    it("invokes set_rating for each selected frame", async () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      const frame1 = { path: "/a/1.jpg", name: "1.jpg", rating: 0 };
      const frame2 = { path: "/a/2.jpg", name: "2.jpg", rating: 0 };
      const view = [frame1, frame2];

      // Mark frame1 as selected in selection store
      const { setSelection } = await import("@modules/core");
      setSelection(new Set(["/a/1.jpg"]));

      await rate(view, 5);

      expect(invoke).toHaveBeenCalledWith("set_rating", { path: "/a/1.jpg", rating: 5 });
      expect(frame1.rating).toBe(5);
    });
  });

  describe("stopCull", () => {
    it("calls invoke cancel_cull", () => {
      vi.mocked(invoke).mockResolvedValue(undefined);
      stopCull();
      expect(invoke).toHaveBeenCalledWith("cancel_cull");
    });
  });

  describe("cull event handlers", () => {
    it("handles cull lifecycle events properly", () => {
      handleCullStarted({ total: 50 });
      expect(cullingState.cullTaskId).toBeTruthy();

      handleCullProgress({ done: 25, total: 50, phase: "cloud" });
      expect(activity.progress?.verb).toBe("cull");
      expect(activity.progress?.done).toBe(25);

      handleCullFinished({ picked: 10, considered: 50, marked: 10 });
      expect(cullingState.cullTaskId).toBeNull();
      expect(activity.progress).toBeNull();
    });

    it("handles cull failed event", () => {
      handleCullStarted({ total: 30 });
      expect(cullingState.cullTaskId).toBeTruthy();

      handleCullFailed({ message: "Network error" });
      expect(cullingState.cullTaskId).toBeNull();
      expect(activity.progress).toBeNull();
    });
  });
});
