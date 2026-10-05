import { describe, it, expect } from "vitest";
import { Source } from "./Source.js";

describe("Source Model", () => {
  describe("Source.fromLocalFolder", () => {
    it("creates a local folder source with formatted name and id", () => {
      const src = Source.fromLocalFolder("/Volumes/NAS/Production/2026", 1420);
      expect(src.id).toBe("local:/Volumes/NAS/Production/2026");
      expect(src.type).toBe("folder");
      expect(src.name).toBe("2026");
      expect(src.path).toBe("/Volumes/NAS/Production/2026");
      expect(src.icon).toBe("folder");
      expect(src.connected).toBe(true);
      expect(src.readOnly).toBe(false);
      expect(src.totalPhotos).toBe(1420);
      expect(src.isLocal).toBe(true);
      expect(src.isRemote).toBe(false);
    });

    it("trims trailing slashes on paths", () => {
      const src = Source.fromLocalFolder("/Volumes/Photos/");
      expect(src.path).toBe("/Volumes/Photos");
      expect(src.name).toBe("Photos");
      expect(src.id).toBe("local:/Volumes/Photos");
    });
  });

  describe("Source.fromApplePhotos", () => {
    it("creates Apple Photos source with correct flags", () => {
      const src = Source.fromApplePhotos({ supported: true, total: 32000, busy: false });
      expect(src.id).toBe("apple-photos");
      expect(src.type).toBe("apple-photos");
      expect(src.name).toBe("Apple Photos");
      expect(src.connected).toBe(true);
      expect(src.readOnly).toBe(true);
      expect(src.totalPhotos).toBe(32000);
      expect(src.status).toBe("idle");
      expect(src.isLocal).toBe(false);
      expect(src.isRemote).toBe(true);
    });

    it("reflects busy status as syncing", () => {
      const src = Source.fromApplePhotos({ supported: true, busy: true });
      expect(src.status).toBe("syncing");
    });
  });

  describe("Source.fromImmich", () => {
    it("creates Immich source with cloud icon and connection state", () => {
      const src = Source.fromImmich({ connected: true, libraryTotal: 8500 });
      expect(src.id).toBe("immich");
      expect(src.type).toBe("immich");
      expect(src.name).toBe("Immich");
      expect(src.icon).toBe("cloud");
      expect(src.connected).toBe(true);
      expect(src.readOnly).toBe(true);
      expect(src.totalPhotos).toBe(8500);
      expect(src.isRemote).toBe(true);
    });
  });
});
