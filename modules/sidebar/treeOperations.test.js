import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  filterGhosts,
  readPersistedSet,
  writePersistedSet,
  buildCatalogueTrees,
} from "./treeOperations.js";

describe("modules/sidebar/treeOperations", () => {
  describe("filterGhosts", () => {
    it("keeps ghosts not present in dirs and drops those that exist", () => {
      const ghosts = [
        { abs: "/root/2026/01", name: "01", parentAbs: "/root/2026" },
        { abs: "/root/2026/02", name: "02", parentAbs: "/root/2026" },
      ];
      const dirs = [{ dir: "/root/2026/01", count: 12 }];

      const result = filterGhosts(ghosts, dirs);
      expect(result).toEqual([
        { abs: "/root/2026/02", name: "02", parentAbs: "/root/2026" },
      ]);
    });
  });

  describe("readPersistedSet and writePersistedSet", () => {
    let store = new Map();

    beforeEach(() => {
      store = new Map();
      globalThis.localStorage = /** @type {any} */ ({
        getItem: (k) => store.get(k) ?? null,
        setItem: (k, v) => store.set(k, String(v)),
        removeItem: (k) => store.delete(k),
        clear: () => store.clear(),
      });
    });

    afterEach(() => {
      // @ts-ignore
      delete globalThis.localStorage;
    });

    it("reads empty set when key is missing or invalid", () => {
      expect(readPersistedSet("missing_key")).toEqual(new Set());

      localStorage.setItem("bad_key", "invalid json");
      expect(readPersistedSet("bad_key")).toEqual(new Set());
    });

    it("round-trips a set through localStorage", () => {
      const original = new Set(["folder-a", "folder-b"]);
      writePersistedSet("test_key", original);

      const restored = readPersistedSet("test_key");
      expect(restored).toEqual(original);
    });
  });

  describe("buildCatalogueTrees", () => {
    it("builds a nested tree for a catalog root and sorts nodes descending", () => {
      const roots = ["/Volumes/Photos"];
      const dirs = [
        { dir: "/Volumes/Photos/2026", count: 0 },
        { dir: "/Volumes/Photos/2026/01", count: 42 },
        { dir: "/Volumes/Photos/2026/02", count: 15 },
        { dir: "/Volumes/Photos/2025", count: 100 },
      ];

      const catalogues = buildCatalogueTrees({
        roots,
        dirs,
        isCatExpanded: () => true,
      });

      expect(catalogues).toHaveLength(1);
      const cat = catalogues[0];
      expect(cat.cat).toBe("/Volumes/Photos");
      expect(cat.name).toBe("Photos");
      expect(cat.total).toBe(157); // 42 + 15 + 100

      // Top level nodes sorted descending: "2026", "2025"
      expect(cat.tree.nodes.map((n) => n.name)).toEqual(["2026", "2025"]);

      const node2026 = cat.tree.nodes[0];
      // Sub-nodes sorted descending: "02", "01"
      expect(node2026.children.map((n) => n.name)).toEqual(["02", "01"]);
      expect(node2026.children[0].count).toBe(15);
      expect(node2026.children[1].count).toBe(42);
    });

    it("calculates total without building tree nodes when catalog is collapsed", () => {
      const roots = ["/Volumes/Archive"];
      const dirs = [
        { dir: "/Volumes/Archive/2024", count: 50 },
        { dir: "/Volumes/Archive/2025", count: 30 },
      ];

      const catalogues = buildCatalogueTrees({
        roots,
        dirs,
        isCatExpanded: () => false,
      });

      expect(catalogues).toHaveLength(1);
      expect(catalogues[0].total).toBe(80);
      expect(catalogues[0].tree.nodes).toEqual([]);
    });

    it("builds trees for multiple local roots", () => {
      const roots = ["/Volumes/Photos", "/Volumes/Archive"];
      const dirs = [
        { dir: "/Volumes/Photos/2026", count: 10 },
        { dir: "/Volumes/Archive/2025", count: 20 },
      ];

      const catalogues = buildCatalogueTrees({
        roots,
        dirs,
      });

      expect(catalogues).toHaveLength(2);
      expect(catalogues[0].cat).toBe("/Volumes/Photos");
      expect(catalogues[0].name).toBe("Photos");
      expect(catalogues[0].total).toBe(10);
      expect(catalogues[1].cat).toBe("/Volumes/Archive");
      expect(catalogues[1].name).toBe("Archive");
      expect(catalogues[1].total).toBe(20);
    });
  });
});
