import { describe, it, expect, vi, beforeEach } from "vitest";
import { createPhotoLoader, previewUrl, openingUrl } from "./photoLoader.js";

describe("photoLoader", () => {
  it("formats previewUrl and openingUrl", () => {
    const pUrl = previewUrl("/path/to/img.raw", 2);
    expect(pUrl).toContain("reveal://thumb");
    expect(pUrl).toContain("size=2048");
    expect(pUrl).toContain("priority=1");

    const oUrl = openingUrl("/path/to/img.raw", 2);
    expect(oUrl).toContain("reveal://thumb");
    expect(oUrl).not.toContain("size=2048");
  });

  it("coordinates photo loading, recipe application, and mode switching", async () => {
    let currentPhotoPath = null;
    let currentPicked = null;
    let currentImgUrl = null;
    let currentStatus = null;
    let currentMode = "cull";
    let spaceLookVal = false;

    const developState = {
      useCanvas: true,
      imgFailed: true,
      caption: "",
      tags: [],
      developEngine: null,
      recipe: null,
      resetHistory: vi.fn(),
    };

    const invoke = vi.fn().mockImplementation(async (cmd) => {
      if (cmd === "load_sidecar") {
        return {
          description: "Test caption",
          tags: ["nature"],
          engine: "rapid",
          engine_settings: { exposure: 0.5 },
        };
      }
      if (cmd === "default_recipe") {
        return { exposure: 0, tint: 0 };
      }
      return {};
    });

    const prefetchNeighbours = vi.fn();
    const scheduleWorkingPark = vi.fn();
    const scheduleRender = vi.fn();
    const switchMode = vi.fn().mockImplementation(async (m) => {
      currentMode = m;
    });
    const notify = vi.fn();

    const loader = createPhotoLoader({
      invoke,
      developState,
      getFrames: () => [{ path: "/photos/img.arw", previewVersion: 3 }],
      getPhotoPath: () => currentPhotoPath,
      setPhotoPath: (p) => { currentPhotoPath = p; },
      setPicked: (p) => { currentPicked = p; },
      getImgUrl: () => currentImgUrl,
      setImgUrl: (url) => { currentImgUrl = url; },
      setStatus: (s) => { currentStatus = s; },
      getPreferences: () => ({ default_engine: "spektra" }),
      prefetchNeighbours,
      scheduleWorkingPark,
      scheduleRender,
      switchMode,
      setSpaceLook: (val) => { spaceLookVal = val; },
      getCurrentMode: () => currentMode,
      notify,
      createImage: () => null,
    });

    await loader.openPhoto("/photos/img.arw", { openDevPanel: false });

    expect(currentPhotoPath).toBe("/photos/img.arw");
    expect(currentPicked).toBe("img.arw");
    expect(developState.useCanvas).toBe(false);
    expect(developState.caption).toBe("Test caption");
    expect(developState.tags).toEqual(["nature"]);
    expect(developState.developEngine).toBe("rapid");
    expect(developState.recipe).toEqual({ exposure: 0.5, tint: 0 });
    expect(developState.resetHistory).toHaveBeenCalled();
    expect(prefetchNeighbours).toHaveBeenCalledWith("/photos/img.arw");
    expect(scheduleWorkingPark).toHaveBeenCalledWith("/photos/img.arw");
    expect(scheduleRender).toHaveBeenCalledWith(2048);
    expect(spaceLookVal).toBe(true);
    expect(switchMode).toHaveBeenCalledWith("dev", { openDevPanel: false });
  });

  it("handles photo loading failure gracefully", async () => {
    let currentStatus = "";
    const notify = vi.fn();
    const invoke = vi.fn().mockRejectedValue(new Error("File not found"));

    const loader = createPhotoLoader({
      invoke,
      developState: {},
      getFrames: () => [],
      getPhotoPath: () => "/missing.raw",
      setPhotoPath: () => {},
      setPicked: () => {},
      getImgUrl: () => null,
      setImgUrl: () => {},
      setStatus: (s) => { currentStatus = s; },
      getPreferences: () => ({}),
      prefetchNeighbours: vi.fn(),
      scheduleWorkingPark: vi.fn(),
      scheduleRender: vi.fn(),
      switchMode: vi.fn(),
      getCurrentMode: () => "cull",
      notify,
      createImage: () => null,
    });

    await loader.openPhoto("/missing.raw");
    expect(currentStatus).toBe("Could not load photo");
    expect(notify).toHaveBeenCalledWith(expect.stringContaining("Could not open photo"), 5000);
  });
});
