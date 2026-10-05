import { describe, it, expect, vi } from "vitest";
import { registerAppEventListeners } from "./appEventListeners.js";

describe("registerAppEventListeners", () => {
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

  it("registers shell and system events", () => {
    const { on, trigger } = makeMockOn();
    const onGardenAccountChanged = vi.fn();
    const onFocusModeChanged = vi.fn();
    const onShellPrefsChanged = vi.fn();
    const onAppError = vi.fn();
    const onLibrariesChanged = vi.fn();
    const onSourceOffline = vi.fn();

    registerAppEventListeners(on, {
      onGardenAccountChanged,
      onFocusModeChanged,
      onShellPrefsChanged,
      onAppError,
      onLibrariesChanged,
      onSourceOffline,
    });

    trigger("garden-account-changed", { username: "alice" });
    expect(onGardenAccountChanged).toHaveBeenCalledWith({ username: "alice" });

    trigger("focus-mode-changed", { enabled: true });
    expect(onFocusModeChanged).toHaveBeenCalledWith({ enabled: true });

    trigger("shell-prefs-changed", { auto_import: true });
    expect(onShellPrefsChanged).toHaveBeenCalledWith({ auto_import: true });

    trigger("app-error", { message: "Disk full" });
    expect(onAppError).toHaveBeenCalledWith({ message: "Disk full" });

    trigger("libraries-changed");
    expect(onLibrariesChanged).toHaveBeenCalled();

    trigger("source-offline");
    expect(onSourceOffline).toHaveBeenCalled();
  });

  it("registers card, import, cull, and progress events", () => {
    const { on, trigger } = makeMockOn();
    const onCardsChanged = vi.fn();
    const onCardMounted = vi.fn();
    const onCardUnmounted = vi.fn();
    const onImportFirstCardRequested = vi.fn();
    const onImportProgress = vi.fn();
    const onImportPreviewReady = vi.fn();
    const onImportStarted = vi.fn();
    const onImportFinished = vi.fn();
    const onImportFailed = vi.fn();
    const onCullStarted = vi.fn();
    const onCullProgress = vi.fn();
    const onCullFinished = vi.fn();
    const onCullFailed = vi.fn();
    const onExportProgress = vi.fn();
    const onPublishProgress = vi.fn();

    registerAppEventListeners(on, {
      onCardsChanged,
      onCardMounted,
      onCardUnmounted,
      onImportFirstCardRequested,
      onImportProgress,
      onImportPreviewReady,
      onImportStarted,
      onImportFinished,
      onImportFailed,
      onCullStarted,
      onCullProgress,
      onCullFinished,
      onCullFailed,
      onExportProgress,
      onPublishProgress,
    });

    trigger("cards-changed", [{ name: "SD1" }]);
    expect(onCardsChanged).toHaveBeenCalledWith([{ name: "SD1" }]);

    trigger("card-mounted", { name: "SD1", dcim: "/Volumes/SD1/DCIM" });
    expect(onCardMounted).toHaveBeenCalledWith({ name: "SD1", dcim: "/Volumes/SD1/DCIM" });

    trigger("card-unmounted", { dcim: "/Volumes/SD1/DCIM" });
    expect(onCardUnmounted).toHaveBeenCalledWith({ dcim: "/Volumes/SD1/DCIM" });

    trigger("import-first-card-requested");
    expect(onImportFirstCardRequested).toHaveBeenCalled();

    trigger("import-progress", { done: 5, total: 10 });
    expect(onImportProgress).toHaveBeenCalledWith({ done: 5, total: 10 });

    trigger("import-preview-ready", { dest: "/photos/p1.raw", version: 2 });
    expect(onImportPreviewReady).toHaveBeenCalledWith({ dest: "/photos/p1.raw", version: 2 });

    trigger("import-started", {});
    expect(onImportStarted).toHaveBeenCalled();

    trigger("import-finished", { folders: ["/photos/2026-10-05"] });
    expect(onImportFinished).toHaveBeenCalledWith({ folders: ["/photos/2026-10-05"] });

    trigger("import-failed", { message: "Read error" });
    expect(onImportFailed).toHaveBeenCalledWith({ message: "Read error" });

    trigger("cull-started", { dir: "/photos" });
    expect(onCullStarted).toHaveBeenCalledWith({ dir: "/photos" });

    trigger("cull-progress", { done: 1, total: 5 });
    expect(onCullProgress).toHaveBeenCalledWith({ done: 1, total: 5 });

    trigger("cull-finished", { success: true });
    expect(onCullFinished).toHaveBeenCalledWith({ success: true });

    trigger("cull-failed", { error: "AI failed" });
    expect(onCullFailed).toHaveBeenCalledWith({ error: "AI failed" });

    trigger("export-progress", { done: 3, total: 4 });
    expect(onExportProgress).toHaveBeenCalledWith({ done: 3, total: 4 });

    trigger("publish-progress", { done: 1, total: 1 });
    expect(onPublishProgress).toHaveBeenCalledWith({ done: 1, total: 1 });
  });

  it("delegates sub-bridges when provided", () => {
    const { on, trigger } = makeMockOn();
    const onDevReady = vi.fn();
    const onSettingsReady = vi.fn();
    const onMenuExport = vi.fn();

    registerAppEventListeners(on, {
      devPanel: { onReady: onDevReady },
      settingsPanel: { onReady: onSettingsReady },
      menu: { onMenuExport },
    });

    trigger("dev-panel-ready");
    expect(onDevReady).toHaveBeenCalled();

    trigger("settings-panel-ready");
    expect(onSettingsReady).toHaveBeenCalled();

    trigger("menu-export-requested");
    expect(onMenuExport).toHaveBeenCalled();
  });
});
