import { describe, it, expect } from "vitest";
import { Collection } from "./Collection.js";

describe("Collection Model", () => {
  describe("Collection.label", () => {
    it("returns empty string if null or undefined", () => {
      expect(Collection.label(null)).toBe("");
      expect(Collection.label(undefined)).toBe("");
    });

    it("returns directory name if no roots are given", () => {
      expect(Collection.label("/Volumes/Photos/2026/Voyage")).toBe("Voyage");
      expect(Collection.label("/Volumes/Photos/2026/Voyage/")).toBe("Voyage");
    });

    it("returns relative path within matching root", () => {
      const roots = ["/Volumes/Photos"];
      expect(Collection.label("/Volumes/Photos/2026/Voyage", roots)).toBe("2026/Voyage");
      expect(Collection.label("/Volumes/Photos", roots)).toBe("Photos");
    });
  });

  describe("Collection.parentOf & ancestors", () => {
    it("finds the direct parent", () => {
      expect(Collection.parentOf("/a/b/c")).toBe("/a/b");
      expect(Collection.parentOf("/a")).toBe(null);
      expect(Collection.parentOf("")).toBe(null);
    });

    it("builds the ancestor chain down to root", () => {
      const list = Collection.ancestors("/Volumes/Photos/2026/09", "/Volumes/Photos");
      expect(list).toEqual(["/Volumes/Photos", "/Volumes/Photos/2026"]);
    });
  });

  describe("Collection.isRoot & storyPath", () => {
    it("checks whether path is one of the roots", () => {
      expect(Collection.isRoot("/Volumes/Photos", ["/Volumes/Photos/", "/Volumes/Work"])).toBe(true);
      expect(Collection.isRoot("/Volumes/Other", ["/Volumes/Photos"])).toBe(false);
    });

    it("formats story note path", () => {
      expect(Collection.storyPath("/Volumes/Photos/2026")).toBe("/Volumes/Photos/2026/.reveal/story.md");
    });
  });

  describe("Collection.fromFolder", () => {
    it("creates a local folder collection", () => {
      const roots = ["/Volumes/NAS"];
      const col = Collection.fromFolder("/Volumes/NAS/2026/Voyage", roots, 42);
      expect(col.id).toBe("/Volumes/NAS/2026/Voyage");
      expect(col.sourceId).toBe("local:/Volumes/NAS");
      expect(col.name).toBe("2026/Voyage");
      expect(col.kind).toBe("folder");
      expect(col.path).toBe("/Volumes/NAS/2026/Voyage");
      expect(col.count).toBe(42);
      expect(col.readOnly).toBe(false);
      expect(col.isFolder).toBe(true);
      expect(col.isVirtual).toBe(false);
      expect(col.isRemote).toBe(false);
    });
  });

  describe("Collection.fromAppleAlbum & fromImmichAlbum", () => {
    it("creates Apple Photos collection", () => {
      const col = Collection.fromAppleAlbum({ id: "favorites-123", title: "Favoris 2026", count: 18 });
      expect(col.id).toBe("apple-photos://album/favorites-123");
      expect(col.sourceId).toBe("apple-photos");
      expect(col.name).toBe("Favoris 2026");
      expect(col.kind).toBe("album");
      expect(col.readOnly).toBe(true);
      expect(col.isRemote).toBe(true);
    });

    it("creates Immich album collection", () => {
      const col = Collection.fromImmichAlbum({ id: "immich-uuid-456", title: "Roadtrip", count: 95 });
      expect(col.id).toBe("immich://album/immich-uuid-456");
      expect(col.sourceId).toBe("immich");
      expect(col.name).toBe("Roadtrip");
      expect(col.icon).toBe("cloud");
      expect(col.readOnly).toBe(true);
      expect(col.isRemote).toBe(true);
    });
  });

  describe("Collection.fromVirtual", () => {
    it("creates virtual smart collection", () => {
      const col = Collection.fromVirtual({
        name: "5 Stars RAW",
        query: { filter: { rating: { gte: 5 }, isRaw: true } },
      });
      expect(col.id).toBe("virtual://5-stars-raw");
      expect(col.sourceId).toBe("reveal");
      expect(col.name).toBe("5 Stars RAW");
      expect(col.kind).toBe("virtual");
      expect(col.isVirtual).toBe(true);
      expect(col.isFolder).toBe(false);
      expect(col.query?.filter?.isRaw).toBe(true);
    });
  });
});
