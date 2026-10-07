import { describe, it, expect, vi } from "vitest";
import { createImportPanelMirror } from "./importPanelMirror.svelte.js";

describe("modules/import/importPanelMirror", () => {
  it("initializes with default state and constructs valid thumbnail URLs", () => {
    const mirror = createImportPanelMirror();

    expect(mirror.cards).toEqual([]);
    expect(mirror.importingCard).toBeNull();
    expect(mirror.ejectableCard).toBeNull();
    expect(mirror.progress).toBeNull();
    expect(mirror.outcome).toBeNull();

    expect(mirror.thumbUrl("/Volumes/SD/DCIM/100EOS/IMG_0001.CR3")).toBe(
      "reveal://thumb?p=%2FVolumes%2FSD%2FDCIM%2F100EOS%2FIMG_0001.CR3&v=0&size=768"
    );
  });

  it("exposes panelProps matching the active mirror state", () => {
    const mirror = createImportPanelMirror();
    const props = mirror.panelProps;

    expect(props.cards).toEqual([]);
    expect(props.importingCard).toBeNull();
    expect(props.headerIcon).toBe("download-simple");
    expect(props.headerText).toBe("Importing");
    expect(typeof props.onStartDrag).toBe("function");
    expect(typeof props.onCancelImport).toBe("function");
    expect(typeof props.onEjectCard).toBe("function");
    expect(typeof props.onOpenReveal).toBe("function");
  });

  it("calls cancel_import when cancelImport is called", async () => {
    const mockInvoke = vi.fn().mockResolvedValue(undefined);
    const mirror = createImportPanelMirror({ invokeFn: mockInvoke });

    await mirror.cancelImport();
    expect(mockInvoke).toHaveBeenCalledWith("cancel_import");
  });

  it("calls eject_card when ejectCard is called with volume", async () => {
    const mockInvoke = vi.fn().mockResolvedValue(undefined);
    const mockHide = vi.fn();
    const mockWindow = { hide: mockHide, startDragging: vi.fn() };
    const mirror = createImportPanelMirror({
      invokeFn: mockInvoke,
      getWindowFn: () => /** @type {any} */ (mockWindow),
    });

    await mirror.ejectCard({
      volume: "/Volumes/EOS_DIGITAL",
      name: "EOS_DIGITAL",
      dcim: "/Volumes/EOS_DIGITAL/DCIM",
      raw_count: 42,
    });

    expect(mockInvoke).toHaveBeenCalledWith("eject_card", { volume: "/Volumes/EOS_DIGITAL" });
    expect(mockHide).toHaveBeenCalled();
  });
});
