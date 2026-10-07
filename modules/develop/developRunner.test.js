import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createPrefetcher, createWorkingParker, createRenderPump } from "./developRunner.js";

describe("developRunner", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe("createPrefetcher", () => {
    it("debounces prefetching of neighbor frames", () => {
      const invoke = vi.fn().mockResolvedValue(undefined);
      const prefetcher = createPrefetcher({
        isTauri: true,
        invoke,
        getPhotoPath: () => "/p2.raw",
        getView: () => [{ path: "/p1.raw" }, { path: "/p2.raw" }, { path: "/p3.raw" }],
        getSel: () => 1,
        getCurrentMode: () => "dev",
      });

      prefetcher.prefetchNeighbours("/p2.raw");
      expect(invoke).not.toHaveBeenCalled();

      vi.advanceTimersByTime(450);
      expect(invoke).toHaveBeenCalledWith("prefetch_photo", { path: "/p3.raw" });
      expect(invoke).toHaveBeenCalledWith("prefetch_photo", { path: "/p1.raw" });
    });

    it("warms selection in cull mode after debounce", () => {
      const invoke = vi.fn().mockResolvedValue(undefined);
      const prefetcher = createPrefetcher({
        isTauri: true,
        invoke,
        getPhotoPath: () => null,
        getView: () => [{ path: "/p1.raw" }, { path: "/p2.raw" }],
        getSel: () => 0,
        getCurrentMode: () => "cull",
      });

      prefetcher.warmSelection("/p1.raw");
      vi.advanceTimersByTime(450);

      expect(invoke).toHaveBeenCalledWith("prefetch_photo", { path: "/p1.raw" });
    });
  });

  describe("createWorkingParker", () => {
    it("schedules park of decoded frame in dev mode", () => {
      const invoke = vi.fn().mockResolvedValue(undefined);
      const parker = createWorkingParker({
        isTauri: true,
        invoke,
        getCurrentMode: () => "dev",
        getPhotoPath: () => "/photo.raw",
      });

      parker.scheduleWorkingPark("/photo.raw");
      vi.advanceTimersByTime(1500);

      expect(invoke).toHaveBeenCalledWith("park_working_frame", { path: "/photo.raw" });
    });

    it("releases parked frame when not in dev mode", () => {
      const invoke = vi.fn().mockResolvedValue(undefined);
      let mode = "dev";
      const parker = createWorkingParker({
        isTauri: true,
        invoke,
        getCurrentMode: () => mode,
        getPhotoPath: () => "/photo.raw",
      });

      parker.scheduleWorkingPark("/photo.raw");
      vi.advanceTimersByTime(1500);

      mode = "cull";
      parker.scheduleWorkingRelease();
      vi.advanceTimersByTime(5000);

      expect(invoke).toHaveBeenCalledWith("release_working_frame");
    });
  });

  describe("createRenderPump", () => {
    it("sets pendingPx and launches pump", async () => {
      const invoke = vi.fn().mockResolvedValue(new Uint8Array(100));
      const developState = {
        inflight: false,
        pendingPx: null,
        pendingLive: false,
        recipe: { engine: "spektra" },
      };
      const onLoupeBlobCreated = vi.fn();
      const setStatus = vi.fn();

      // Mock global URL and Image
      globalThis.URL.createObjectURL = vi.fn().mockReturnValue("blob:mock");
      globalThis.URL.revokeObjectURL = vi.fn();
      class MockImage {
        decode() { return Promise.resolve(); }
      }
      globalThis.Image = /** @type {any} */ (MockImage);

      const pump = createRenderPump({
        developState,
        invoke,
        tick: () => Promise.resolve(),
        getPhotoPath: () => "/photo.raw",
        isDockedCrop: () => false,
        library: { frames: [] },
        freshPreviewVersion: () => Promise.resolve(1),
        refreshFrames: () => {},
        onLoupeBlobCreated,
        setStatus,
      });

      await pump.scheduleRender(1024, false);

      expect(invoke).toHaveBeenCalledWith("develop_preview", {
        path: "/photo.raw",
        recipe: { engine: "spektra", apply_crop: true },
        maxPx: 1024,
        live: false,
      });
      expect(onLoupeBlobCreated).toHaveBeenCalledWith("blob:mock");
      expect(developState.inflight).toBe(false);
    });
  });
});
