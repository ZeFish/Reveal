import { describe, it, expect } from "vitest";
import { Destination } from "./Destination.js";

describe("Destination Model", () => {
  describe("Destination.localFolder", () => {
    it("creates a local folder destination with fallback to Desktop", () => {
      const dest = Destination.localFolder();
      expect(dest.id).toBe("folder:desktop");
      expect(dest.kind).toBe("folder");
      expect(dest.name).toBe("Desktop");
      expect(dest.icon).toBe("folder");
      expect(dest.isLocal).toBe(true);
      expect(dest.isRemote).toBe(false);
      expect(dest.ready).toBe(true);
      expect(dest.options.longEdge).toBe(2048);
    });

    it("creates a custom folder destination", () => {
      const dest = Destination.localFolder("/Volumes/Export/Web", { longEdge: 1600, border: true });
      expect(dest.id).toBe("folder:/Volumes/Export/Web");
      expect(dest.name).toBe("Web");
      expect(dest.path).toBe("/Volumes/Export/Web");
      expect(dest.options.longEdge).toBe(1600);
      expect(dest.options.border).toBe(true);
    });
  });

  describe("Destination.obsidian", () => {
    it("creates an Obsidian daily note destination", () => {
      const dest = Destination.obsidian({ longEdge: 2048, border: true });
      expect(dest.id).toBe("obsidian:daily-note");
      expect(dest.kind).toBe("obsidian");
      expect(dest.name).toBe("Obsidian Daily Note");
      expect(dest.icon).toBe("book-bookmark");
      expect(dest.isObsidian).toBe(true);
      expect(dest.isLocal).toBe(false);
    });
  });

  describe("Destination.garden", () => {
    it("creates a Garden publication destination", () => {
      const dest = Destination.garden({ gardenUrl: "https://stnd.gd/@francis", allowDownload: true, signedIn: true });
      expect(dest.id).toBe("garden:publication");
      expect(dest.kind).toBe("garden");
      expect(dest.name).toBe("Garden");
      expect(dest.icon).toBe("plant");
      expect(dest.path).toBe("https://stnd.gd/@francis");
      expect(dest.isGarden).toBe(true);
      expect(dest.isRemote).toBe(true);
      expect(dest.options.allowDownload).toBe(true);
      expect(dest.ready).toBe(true);
    });
  });

  describe("Destination.editor", () => {
    it("creates an external editor destination", () => {
      const dest = Destination.editor("Affinity Photo", "/Applications/Affinity Photo 2.app");
      expect(dest.id).toBe("editor:affinity-photo");
      expect(dest.kind).toBe("editor");
      expect(dest.name).toBe("Affinity Photo");
      expect(dest.path).toBe("/Applications/Affinity Photo 2.app");
      expect(dest.isEditor).toBe(true);
      expect(dest.ready).toBe(true);
    });
  });

  describe("Destination.immich", () => {
    it("creates an Immich remote destination", () => {
      const dest = Destination.immich({ albumId: "abc-123", albumTitle: "Vacances 2026" });
      expect(dest.id).toBe("immich:album:abc-123");
      expect(dest.kind).toBe("remote");
      expect(dest.name).toBe("Vacances 2026");
      expect(dest.icon).toBe("cloud");
      expect(dest.isRemote).toBe(true);
    });
  });
});
