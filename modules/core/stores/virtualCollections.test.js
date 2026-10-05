import { describe, it, expect, beforeEach } from "vitest";
import { virtualCollections, SMART_COLLECTIONS } from "./virtualCollections.svelte.js";

describe("virtualCollections store", () => {
  beforeEach(() => {
    // Clear custom user collections
    for (const c of virtualCollections.all) {
      if (c.id.startsWith("virtual://curated/")) {
        virtualCollections.remove(c.id);
      }
    }
  });

  it("provides default smart collections", () => {
    expect(virtualCollections.all.length).toBeGreaterThanOrEqual(3);
    const ids = virtualCollections.all.map((c) => c.id);
    expect(ids).toContain("virtual://favorites");
    expect(ids).toContain("virtual://picks");
    expect(ids).toContain("virtual://story");
  });

  it("detects virtual collection IDs", () => {
    expect(virtualCollections.isVirtualId("virtual://favorites")).toBe(true);
    expect(virtualCollections.isVirtualId("virtual://curated/test")).toBe(true);
    expect(virtualCollections.isVirtualId("/Volumes/Photos")).toBe(false);
    expect(virtualCollections.isVirtualId(null)).toBe(false);
  });

  it("creates a curated collection and adds photos (drag & drop simulation)", () => {
    const col = virtualCollections.create("Exposition 2026");
    expect(col.name).toBe("Exposition 2026");
    expect(virtualCollections.get(col.id)).toBeDefined();

    const added = virtualCollections.addPhotos(col.id, ["/photos/a.jpg", "/photos/b.jpg"]);
    expect(added).toBe(2);

    const updated = virtualCollections.get(col.id);
    expect(updated?.count).toBe(2);
    expect(updated?.query?.uris).toEqual(["/photos/a.jpg", "/photos/b.jpg"]);

    // Deduplication on secondary drop
    const addedAgain = virtualCollections.addPhotos(col.id, ["/photos/b.jpg", "/photos/c.jpg"]);
    expect(addedAgain).toBe(1);
    expect(virtualCollections.get(col.id)?.count).toBe(3);
  });

  it("removes a curated collection", () => {
    const col = virtualCollections.create("Temporaire");
    expect(virtualCollections.get(col.id)).toBeDefined();

    virtualCollections.remove(col.id);
    expect(virtualCollections.get(col.id)).toBeUndefined();
  });

  it("filters frames based on smart query or curated set", () => {
    const frames = [
      { path: "/photos/a.jpg", rating: 5, pick: "picked" },
      { path: "/photos/b.jpg", rating: 3, pick: "none" },
      { path: "/photos/c.jpg", rating: 5, pick: "none" },
    ];

    const favs = virtualCollections.get("virtual://favorites");
    expect(favs).toBeDefined();
    const favFrames = virtualCollections.filterFrames(favs, frames);
    expect(favFrames.map((f) => f.path)).toEqual(["/photos/a.jpg", "/photos/c.jpg"]);

    const picks = virtualCollections.get("virtual://picks");
    expect(picks).toBeDefined();
    const pickFrames = virtualCollections.filterFrames(picks, frames);
    expect(pickFrames.map((f) => f.path)).toEqual(["/photos/a.jpg"]);
  });

  it("calculates live counts for smart collections with withCounts", () => {
    const frames = [
      { path: "/photos/a.jpg", name: "a.jpg", rating: 5, pick: "picked" },
      { path: "/photos/b.jpg", name: "b.jpg", rating: 3, pick: "none" },
      { path: "/photos/c.jpg", name: "c.jpg", rating: 5, pick: "none" },
    ];
    const storySet = new Set(["b"]);

    const cols = virtualCollections.withCounts(frames, storySet);
    const favCol = cols.find((c) => c.id === "virtual://favorites");
    const pickCol = cols.find((c) => c.id === "virtual://picks");
    const storyCol = cols.find((c) => c.id === "virtual://story");

    expect(favCol?.count).toBe(2);
    expect(pickCol?.count).toBe(1);
    expect(storyCol?.count).toBe(1);
  });
});
