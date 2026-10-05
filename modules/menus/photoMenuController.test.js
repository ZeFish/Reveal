import { describe, it, expect, vi } from "vitest";
import { createPhotoMenuController } from "./photoMenuController.svelte.js";

describe("photoMenuController", () => {
  it("opens, focuses, and closes photo context menu", () => {
    const focusAt = vi.fn();
    const selectOnly = vi.fn();
    const controller = createPhotoMenuController({
      invoke: vi.fn(),
    });

    const view = [{ path: "/photos/1.jpg", name: "1.jpg" }];
    const selection = { paths: new Set() };
    const event = { clientX: 100, clientY: 200, preventDefault: vi.fn() };

    controller.openPhotoMenu(0, event, view, selection, focusAt, selectOnly);
    expect(event.preventDefault).toHaveBeenCalled();
    expect(selectOnly).toHaveBeenCalledWith(view, 0);
    expect(focusAt).toHaveBeenCalledWith(view, 0);
    expect(controller.photoMenu).toEqual({
      frame: view[0],
      x: 100,
      y: 200,
    });

    controller.closePhotoMenu();
    expect(controller.photoMenu).toBeNull();
  });

  it("handles reveal in finder and editor opening", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const hold = vi.fn();
    const controller = createPhotoMenuController({
      invoke,
      hold,
    });

    await controller.revealPhotoInFinder("/photos/1.jpg");
    expect(invoke).toHaveBeenCalledWith("reveal_in_finder", { path: "/photos/1.jpg" });

    await controller.openPhotoInEditor("/photos/1.jpg", "/Applications/Photoshop.app");
    expect(invoke).toHaveBeenCalledWith("open_in_editor", {
      filePath: "/photos/1.jpg",
      appPath: "/Applications/Photoshop.app",
    });
  });

  it("opens photo context menu using bound getters", () => {
    const focusAt = vi.fn();
    const selectOnly = vi.fn();
    const view = [{ path: "/photos/2.jpg", name: "2.jpg" }];
    const selection = { paths: new Set(["/photos/2.jpg"]) };
    const event = { clientX: 50, clientY: 75, preventDefault: vi.fn() };

    const controller = createPhotoMenuController({
      invoke: vi.fn(),
      getView: () => view,
      getSelection: () => selection,
      focusAt,
      selectOnly,
    });

    controller.openPhotoMenu(0, event);
    expect(event.preventDefault).toHaveBeenCalled();
    expect(selectOnly).not.toHaveBeenCalled();
    expect(focusAt).toHaveBeenCalledWith(view, 0);
    expect(controller.photoMenu?.frame).toBe(view[0]);
  });
});
