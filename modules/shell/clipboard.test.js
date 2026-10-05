import { describe, it, expect, vi } from "vitest";
import { copyImageToClipboard, showCopiedMessage } from "./clipboard.js";

describe("shell/clipboard", () => {
  it("showCopiedMessage formats notification with filename", () => {
    const notify = vi.fn();
    showCopiedMessage("/path/to/DSC_0001.JPG", notify);
    expect(notify).toHaveBeenCalledWith(
      "Image copied to the clipboard (DSC_0001.JPG) ✓",
      2500
    );
  });

  it("copies photo preview to clipboard via invoke", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const notify = vi.fn();

    await copyImageToClipboard("/photos/DSC_0002.ARW", {
      currentMode: "cull",
      invoke,
      notify,
    });

    expect(invoke).toHaveBeenCalledWith("copy_photo_preview_to_clipboard", {
      path: "/photos/DSC_0002.ARW",
    });
    expect(notify).toHaveBeenCalledWith(
      "Image copied to the clipboard (DSC_0002.ARW) ✓",
      2500
    );
  });

  it("copies developed preview when in dev mode on active photo", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const notify = vi.fn();
    const recipe = { exposure: 0.5 };

    await copyImageToClipboard("/photos/DSC_0003.ARW", {
      currentMode: "dev",
      photoPath: "/photos/DSC_0003.ARW",
      developState: { recipe, useCanvas: false },
      invoke,
      notify,
      previewPx: 2048,
    });

    expect(invoke).toHaveBeenCalledWith("copy_developed_preview_to_clipboard", {
      path: "/photos/DSC_0003.ARW",
      recipe,
      maxPx: 2048,
    });
  });
});
