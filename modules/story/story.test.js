import { describe, it, expect, vi, beforeEach } from "vitest";
import { storyState } from "./storyState.svelte.js";
import {
  loadStory,
  saveStoryContent,
  refreshStory,
  refreshStoryDirs,
  toggleStoryWithPath,
  buildGridProse,
} from "./storyOperations.js";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@modules/core", async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...actual,
    notify: vi.fn(),
    hold: vi.fn(),
    setProgress: vi.fn(),
    startActivity: vi.fn(() => "task-1"),
    updateActivity: vi.fn(),
    releaseActive: vi.fn(),
    activity: { anyRunning: false },
  };
});

import { invoke } from "@tauri-apps/api/core";

describe("modules/story", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    storyState.storySet = new Set();
    storyState.storyDirs = new Set();
    storyState.filterStory = false;
    storyState.liveUrl = null;
    storyState.storyContent = "";
    storyState.storyRemote = null;
  });

  describe("storyState derived values", () => {
    it("derives gardenUrl from frontmatter or liveUrl", () => {
      expect(storyState.gardenUrl).toBeNull();

      storyState.liveUrl = "https://stnd.gd/s/test";
      expect(storyState.gardenUrl).toBe("https://stnd.gd/s/test");

      storyState.storyContent = "---\ngarden_url: https://stnd.gd/s/frontmatter\n---\n# Title";
      expect(storyState.gardenUrl).toBe("https://stnd.gd/s/frontmatter");
    });

    it("derives publish status and verb", () => {
      expect(storyState.storyPublished).toBe(false);
      expect(storyState.publishVerb).toBe("Publish");

      storyState.storyRemote = { published: true, slug: "test", url: "https://stnd.gd/s/test", updated_at: null };
      expect(storyState.storyPublished).toBe(true);
      expect(storyState.publishVerb).toBe("Update");
    });
  });

  describe("storyOperations", () => {
    it("loadStory fetches note and updates storyContent", async () => {
      vi.mocked(invoke).mockResolvedValueOnce("# My Story\n\n![[IMG_001.jpg]]");

      await loadStory("/Photos/2026-10-04");

      expect(invoke).toHaveBeenCalledWith("load_story_note", { dir: "/Photos/2026-10-04" });
      expect(storyState.storyContent).toBe("# My Story\n\n![[IMG_001.jpg]]");
    });

    it("refreshStory fetches stems and note content", async () => {
      vi.mocked(invoke).mockImplementation((cmd) => {
        if (cmd === "story_stems") return Promise.resolve(["IMG_001", "IMG_002"]);
        if (cmd === "load_story_note") return Promise.resolve("content");
        return Promise.resolve();
      });

      await refreshStory("/Photos/2026-10-04");

      expect(storyState.storySet).toEqual(new Set(["IMG_001", "IMG_002"]));
      expect(storyState.storyContent).toBe("content");
    });

    it("refreshStoryDirs updates storyDirs set", async () => {
      vi.mocked(invoke).mockResolvedValueOnce(["/Photos/Day1", "/Photos/Day2"]);

      await refreshStoryDirs([{ dir: "/Photos/Day1" }, { dir: "/Photos/Day2" }]);

      expect(invoke).toHaveBeenCalledWith("story_dirs", { dirs: ["/Photos/Day1", "/Photos/Day2"] });
      expect(storyState.storyDirs).toEqual(new Set(["/Photos/Day1", "/Photos/Day2"]));
    });

    it("toggleStoryWithPath toggles photo stem and reloads", async () => {
      vi.mocked(invoke).mockImplementation((cmd) => {
        if (cmd === "story_toggle") return Promise.resolve(["IMG_001"]);
        if (cmd === "load_story_note") return Promise.resolve("new content");
        return Promise.resolve();
      });

      const onRefreshed = vi.fn();
      await toggleStoryWithPath("/Photos/IMG_001.JPG", "/Photos", onRefreshed);

      expect(invoke).toHaveBeenCalledWith("story_toggle", { dir: "/Photos", path: "/Photos/IMG_001.JPG" });
      expect(storyState.storySet).toEqual(new Set(["IMG_001"]));
      expect(onRefreshed).toHaveBeenCalled();
    });

    it("buildGridProse correctly maps prose blocks to rows", () => {
      const blocks = [
        { isPhoto: true, stem: "IMG_001" },
        { isPhoto: false, id: "p1", text: "Prose under first photo" },
      ];
      const view = [{ name: "IMG_001.JPG" }, { name: "IMG_002.JPG" }];
      const cols = 2;

      const proseMap = buildGridProse(blocks, view, cols);

      expect(proseMap.has(0)).toBe(true);
      expect(proseMap.get(0)).toEqual([{ id: "p1", text: "Prose under first photo" }]);
    });

    it("coordinates story operations through createStoryController", async () => {
      const { createStoryController } = await import("./storyController.js");
      vi.mocked(invoke).mockImplementation(async (cmd) => {
        if (cmd === "load_story_note") return "# Story Note";
        if (cmd === "story_stems") return ["IMG_001"];
        if (cmd === "story_toggle") return undefined;
        return undefined;
      });

      const onRefreshed = vi.fn();
      const ctrl = createStoryController({
        getDir: () => "/Photos",
        getView: () => [{ path: "/Photos/IMG_001.JPG", name: "IMG_001.JPG" }],
        getSel: () => 0,
        getCols: () => 3,
        getSignedIn: () => true,
        getExportState: () => ({ edge: 2048, border: false }),
        onRefreshed,
      });

      await ctrl.loadStory();
      expect(storyState.storyContent).toBe("# Story Note");

      await ctrl.toggleStory();
      expect(invoke).toHaveBeenCalledWith("story_toggle", { dir: "/Photos", path: "/Photos/IMG_001.JPG" });
      expect(onRefreshed).toHaveBeenCalled();
    });
  });

  describe("syncFolderTheme", () => {
    it("loads and applies folder theme", async () => {
      const { syncFolderTheme, storyTheme } = await import("./storyTheme.svelte.js");
      const invokeMock = vi.fn().mockResolvedValue({ theme: "forest" });
      const applyTheme = vi.fn();

      await syncFolderTheme("/Photos/Summer", {
        isTauri: true,
        invoke: invokeMock,
        applyTheme,
      });

      expect(invokeMock).toHaveBeenCalledWith("story_load_theme", { dir: "/Photos/Summer" });
      expect(storyTheme.id).toBe("forest");
      expect(applyTheme).toHaveBeenCalledWith("forest");
    });

    it("resets theme when dir is null", async () => {
      const { syncFolderTheme, storyTheme } = await import("./storyTheme.svelte.js");
      const applyTheme = vi.fn();

      await syncFolderTheme(null, { isTauri: true, applyTheme });
      expect(storyTheme.id).toBeNull();
      expect(applyTheme).toHaveBeenCalledWith(null);
    });
  });

  describe("watchPublishStatus", () => {
    it("debounces checkStatus call", async () => {
      vi.useFakeTimers();
      const { watchPublishStatus } = await import("./storyOperations.js");
      const checkStatus = vi.fn().mockResolvedValue(null);

      const cleanup = watchPublishStatus("/Photos/Trip", {
        isTauri: true,
        count: 5,
        signedIn: true,
        checkStatus,
        delayMs: 200,
      });

      expect(checkStatus).not.toHaveBeenCalled();
      vi.advanceTimersByTime(250);
      expect(checkStatus).toHaveBeenCalledWith("/Photos/Trip", true);

      cleanup();
      vi.useRealTimers();
    });
  });
});

