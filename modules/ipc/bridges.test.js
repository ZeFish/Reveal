import { describe, it, expect, vi } from "vitest";
import { registerDevPanelBridge } from "./devPanelBridge.js";
import { registerSettingsPanelBridge } from "./settingsPanelBridge.js";
import { registerMenuBridge } from "./menuBridge.js";

describe("IPC Bridges", () => {
  function makeMockOn() {
    /** @type {Record<string, Function>} */
    const listeners = {};
    const on = vi.fn((event, handler) => {
      listeners[event] = handler;
    });
    const trigger = (event, payload) => {
      if (listeners[event]) listeners[event]({ payload });
    };
    return { on, listeners, trigger };
  }

  describe("devPanelBridge", () => {
    it("registers dev panel events and invokes handlers", () => {
      const { on, trigger } = makeMockOn();
      const onReady = vi.fn();
      const onRecipeUpdated = vi.fn();
      const onPhotoScaleChanged = vi.fn();
      const onSetZoneMask = vi.fn();

      registerDevPanelBridge(on, {
        onReady,
        onRecipeUpdated,
        onPhotoScaleChanged,
        onSetZoneMask,
      });

      expect(on).toHaveBeenCalledWith("dev-panel-ready", expect.any(Function));
      expect(on).toHaveBeenCalledWith("dev-panel-recipe-updated", expect.any(Function));

      trigger("dev-panel-ready");
      expect(onReady).toHaveBeenCalled();

      trigger("dev-panel-recipe-updated", { key: "contrast", recipe: { contrast: 5 } });
      expect(onRecipeUpdated).toHaveBeenCalledWith({ key: "contrast", recipe: { contrast: 5 } });

      trigger("dev-panel-photo-scale-changed", { photoScale: 110 });
      expect(onPhotoScaleChanged).toHaveBeenCalledWith(110);

      trigger("dev-panel-set-zone-mask", { zoneMask: "shadows" });
      expect(onSetZoneMask).toHaveBeenCalledWith("shadows");
    });
  });

  describe("settingsPanelBridge", () => {
    it("registers settings panel events and invokes handlers", () => {
      const { on, trigger } = makeMockOn();
      const onReady = vi.fn();
      const onChooseFolder = vi.fn();

      registerSettingsPanelBridge(on, { onReady, onChooseFolder });

      trigger("settings-panel-ready");
      expect(onReady).toHaveBeenCalled();

      trigger("settings-panel-choose-folder", { key: "archive" });
      expect(onChooseFolder).toHaveBeenCalledWith("archive");
    });
  });

  describe("menuBridge", () => {
    it("registers menu events and invokes handlers", () => {
      const { on, trigger } = makeMockOn();
      const onMenuExport = vi.fn();
      const onOpenFile = vi.fn();

      registerMenuBridge(on, { onMenuExport, onOpenFile });

      trigger("menu-export-requested");
      expect(onMenuExport).toHaveBeenCalled();

      trigger("open-file-requested", "/path/to/photo.raw");
      expect(onOpenFile).toHaveBeenCalledWith("/path/to/photo.raw");
    });
  });
});
