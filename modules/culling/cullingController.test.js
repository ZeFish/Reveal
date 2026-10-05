import { describe, it, expect, vi, beforeEach } from "vitest";
import { createCullingController } from "./cullingController.js";
import { cullingState } from "./cullingState.svelte.js";
import * as cullingOps from "./cullingOperations.js";

vi.mock("./cullingOperations.js", () => ({
  saveGridPrefs: vi.fn(),
  toggleLayout: vi.fn(),
  rate: vi.fn(),
  stopCull: vi.fn(),
  triggerAiCull: vi.fn(),
  cullCurrentFolder: vi.fn(),
}));

describe("cullingController", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cullingState.minRating = 0;
  });

  it("coordinates saveGridPrefs and toggleLayout", () => {
    const mockSession = { setGridPrefs: vi.fn() };
    const mockFrames = [{ path: "/p1.jpg" }, { path: "/p2.jpg" }];
    const ctrl = createCullingController({
      session: mockSession,
      getView: () => mockFrames,
      getFrames: () => mockFrames,
      getCurrentDir: () => "/dir",
    });

    ctrl.saveGridPrefs();
    expect(cullingOps.saveGridPrefs).toHaveBeenCalledWith(mockSession);

    ctrl.toggleLayout();
    expect(cullingOps.toggleLayout).toHaveBeenCalledWith({
      totalFrames: 2,
      session: mockSession,
    });
  });

  it("coordinates rate and setMinRating", () => {
    const openDirMock = vi.fn();
    const ctrl = createCullingController({
      getView: () => [{ path: "/p1.jpg" }],
      getCurrentDir: () => "/active/folder",
      openDir: openDirMock,
    });

    ctrl.rate(4);
    expect(cullingOps.rate).toHaveBeenCalledWith([{ path: "/p1.jpg" }], 4);

    ctrl.setMinRating(3);
    expect(cullingState.minRating).toBe(3);
    expect(openDirMock).toHaveBeenCalledWith("/active/folder", true, false, true);
  });

  it("coordinates triggerAiCull and cullCurrentFolder with story refreshes", async () => {
    const refreshStoryMock = vi.fn();
    const refreshStoryDirsMock = vi.fn();
    const loadStoryMock = vi.fn();

    const ctrl = createCullingController({
      getView: () => [{ path: "/p1.jpg" }],
      getCurrentDir: () => "/target/folder",
      refreshStory: refreshStoryMock,
      refreshStoryDirs: refreshStoryDirsMock,
      loadStory: loadStoryMock,
      isApplePhotosActive: () => false,
    });

    ctrl.triggerAiCull("/target/folder", ["/target/folder/p1.jpg"]);
    expect(cullingOps.triggerAiCull).toHaveBeenCalled();

    // Invoke the callback passed to triggerAiCull
    const aiCallback = vi.mocked(cullingOps.triggerAiCull).mock.calls[0][2];
    await aiCallback();
    expect(refreshStoryMock).toHaveBeenCalled();
    expect(refreshStoryDirsMock).toHaveBeenCalled();

    ctrl.cullCurrentFolder();
    expect(cullingOps.cullCurrentFolder).toHaveBeenCalledWith(
      expect.objectContaining({
        dir: "/target/folder",
        isApplePhotos: false,
      })
    );

    const onStoryRefreshed = vi.mocked(cullingOps.cullCurrentFolder).mock.calls[0][0].onStoryRefreshed;
    await onStoryRefreshed();
    expect(loadStoryMock).toHaveBeenCalled();
  });
});
