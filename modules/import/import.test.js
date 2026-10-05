import { describe, it, expect, vi, beforeEach } from "vitest";
import { importState } from "./importState.svelte.js";
import {
  pollCards,
  stopImport,
  setImportDir,
  ejectCard,
  handleCardMounted,
  handleCardUnmounted,
  importCard,
} from "./importOperations.js";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@modules/core", async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...actual,
    notify: vi.fn(),
    hold: vi.fn(),
    setProgress: vi.fn(),
    activity: { progress: null },
  };
});

import { invoke } from "@tauri-apps/api/core";
import { notify, hold, setProgress } from "@modules/core";

describe("modules/import", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    importState.cards = [];
    importState.importingCard = null;
    importState.ejectableCard = null;
    importState.ejecting = false;
    importState.autoImport = false;
    importState.importDir = null;
    importState.lastImportedFolder = null;
  });

  describe("pollCards", () => {
    it("fetches cards and updates importState.cards", async () => {
      const mockCards = [
        { volume: "/Volumes/SD_CARD", name: "EOS_DIGITAL", dcim: "/Volumes/SD_CARD/DCIM", raw_count: 42 },
      ];
      vi.mocked(invoke).mockResolvedValueOnce(mockCards);

      const result = await pollCards();

      expect(invoke).toHaveBeenCalledWith("find_cards");
      expect(result).toEqual(mockCards);
      expect(importState.cards).toEqual(mockCards);
    });
  });

  describe("setImportDir", () => {
    it("calls set_import_dir and notifies with folder name", async () => {
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      await setImportDir("/Users/test/Pictures/2026");

      expect(invoke).toHaveBeenCalledWith("set_import_dir", { path: "/Users/test/Pictures/2026" });
      expect(notify).toHaveBeenCalledWith("Import → 2026", 2500);
    });

    it("holds error on failure", async () => {
      vi.mocked(invoke).mockRejectedValueOnce(new Error("Permission denied"));

      await setImportDir("/invalid");

      expect(hold).toHaveBeenCalledWith(expect.stringContaining("Permission denied"));
    });
  });

  describe("stopImport", () => {
    it("invokes cancel_import", () => {
      vi.mocked(invoke).mockResolvedValueOnce(undefined);

      stopImport();

      expect(invoke).toHaveBeenCalledWith("cancel_import");
    });
  });

  describe("ejectCard", () => {
    it("ejects a card with volume and refreshes cards", async () => {
      const card = { volume: "/Volumes/SD", name: "SD", dcim: "/Volumes/SD/DCIM", raw_count: 10 };
      importState.ejectableCard = card;
      vi.mocked(invoke).mockResolvedValueOnce(undefined); // eject_card
      vi.mocked(invoke).mockResolvedValueOnce([]); // find_cards

      await ejectCard(card);

      expect(invoke).toHaveBeenCalledWith("eject_card", { volume: "/Volumes/SD" });
      expect(importState.ejectableCard).toBeNull();
      expect(importState.ejecting).toBe(false);
      expect(notify).toHaveBeenCalledWith("SD ejected · you can remove the card", 5000);
    });

    it("ignores card without volume or when already ejecting", async () => {
      const cardNoVol = { name: "NoVol", dcim: "/path", raw_count: 0 };
      await ejectCard(cardNoVol);
      expect(invoke).not.toHaveBeenCalled();

      importState.ejecting = true;
      const card = { volume: "/Volumes/SD", name: "SD", dcim: "/Volumes/SD/DCIM", raw_count: 10 };
      await ejectCard(card);
      expect(invoke).not.toHaveBeenCalled();
    });
  });

  describe("handleCardMounted / handleCardUnmounted", () => {
    it("notifies when card mounted and invokes auto-import if enabled", () => {
      const onAuto = vi.fn();
      importState.autoImport = true;
      const card = { name: "SD1", dcim: "/sd1", raw_count: 5 };

      handleCardMounted(card, { onAutoImport: onAuto });

      expect(notify).toHaveBeenCalledWith("card detected · SD1 (5)", 4000);
      expect(onAuto).toHaveBeenCalledWith(card);
    });

    it("clears ejectable card if unmounted dcim matches", () => {
      const card = { name: "SD", dcim: "/Volumes/SD/DCIM", raw_count: 5 };
      importState.ejectableCard = card;

      handleCardUnmounted("/Volumes/OTHER/DCIM");
      expect(importState.ejectableCard).toEqual(card);

      handleCardUnmounted("/Volumes/SD/DCIM");
      expect(importState.ejectableCard).toBeNull();
    });
  });

  describe("importCard", () => {
    it("handles card import flow, scans folder and updates state", async () => {
      const card = { volume: "/Volumes/SD", name: "SD", dcim: "/Volumes/SD/DCIM", raw_count: 10 };
      const onFinishedFolder = vi.fn();
      const onRefreshDirs = vi.fn();

      vi.mocked(invoke).mockImplementation((cmd) => {
        if (cmd === "import_card") {
          return Promise.resolve({
            cancelled: false,
            copied: 10,
            skipped: 0,
            failed: 0,
            folders: ["/Archive/2026-10-04"],
          });
        }
        if (cmd === "scan_folder") return Promise.resolve();
        if (cmd === "notify_user") return Promise.resolve();
        if (cmd === "find_cards") return Promise.resolve([]);
        return Promise.resolve();
      });

      await importCard(card, {
        archive: "/Archive",
        onFinishedFolder,
        onRefreshDirs,
      });

      expect(setProgress).toHaveBeenCalledWith({ verb: "import", done: 0, total: 10, current: "" });
      expect(invoke).toHaveBeenCalledWith("import_card", { dcim: "/Volumes/SD/DCIM", archive: "/Archive" });
      expect(invoke).toHaveBeenCalledWith("scan_folder", { path: "/Archive" });
      expect(onFinishedFolder).toHaveBeenCalledWith("/Archive/2026-10-04");
      expect(importState.lastImportedFolder).toBe("/Archive/2026-10-04");
      expect(importState.ejectableCard).toEqual(card);
      expect(importState.importingCard).toBeNull();
    });
  });
});
