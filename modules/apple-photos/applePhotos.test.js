import { describe, it, expect } from "vitest";
import { photoIdentity, restorePhotoSelection } from "./applePhotosBrowsing.js";
import { applePhotos, leaveApplePhotos } from "./applePhotos.svelte.js";

describe("applePhotos vertical slice", () => {
  it("photoIdentity strips apple-photos protocol and returns asset id", () => {
    expect(photoIdentity("apple-photos://ABC-123/full.jpg")).toBe("ABC-123");
    expect(photoIdentity("/Volumes/Photos/2026/DSC001.ARW")).toBe("/Volumes/Photos/2026/DSC001.ARW");
  });

  it("restorePhotoSelection preserves focus and selections by identity", () => {
    const rows = [
      { path: "apple-photos://A/1.jpg" },
      { path: "apple-photos://B/2.jpg" },
      { path: "apple-photos://C/3.jpg" },
    ];
    const previous = {
      selected: new Set(["apple-photos://B/2.jpg"]),
      focus: "apple-photos://B/2.jpg",
      anchor: "apple-photos://B/2.jpg",
      index: 1,
    };
    const restored = restorePhotoSelection(rows, previous);
    expect(restored.index).toBe(1);
    expect(restored.selected.has("apple-photos://B/2.jpg")).toBe(true);
  });

  it("initial state is inactive", () => {
    expect(applePhotos.active).toBe(false);
  });

  it("leaveApplePhotos increments request and deactivates", () => {
    const reqBefore = applePhotos.request;
    leaveApplePhotos();
    expect(applePhotos.request).toBe(reqBefore + 1);
    expect(applePhotos.active).toBe(false);
    expect(applePhotos.busy).toBe(false);
  });
});
