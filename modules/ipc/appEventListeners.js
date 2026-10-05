import { registerDevPanelBridge } from "./devPanelBridge.js";
import { registerSettingsPanelBridge } from "./settingsPanelBridge.js";
import { registerMenuBridge } from "./menuBridge.js";

/**
 * @typedef {Object} AppEventListenerHandlers
 * @property {(account: any) => void} [onGardenAccountChanged]
 * @property {(payload: { enabled: boolean }) => void} [onFocusModeChanged]
 * @property {(prefs: any) => void} [onShellPrefsChanged]
 * @property {(progress: any) => void} [onIndexProgress]
 * @property {(cards: any[]) => void} [onCardsChanged]
 * @property {(card: any) => void} [onCardMounted]
 * @property {(payload: any) => void} [onCardUnmounted]
 * @property {() => void} [onImportFirstCardRequested]
 * @property {() => void} [onLibrariesChanged]
 * @property {() => void} [onSourceOffline]
 * @property {(error: any) => void} [onAppError]
 * @property {(payload: any) => void} [onImportProgress]
 * @property {(payload: any) => void} [onImportPreviewReady]
 * @property {(payload: any) => void} [onImportStarted]
 * @property {(stats: any) => void} [onImportFinished]
 * @property {(err: any) => void} [onImportFailed]
 * @property {(payload: any) => void} [onCullStarted]
 * @property {(payload: any) => void} [onCullProgress]
 * @property {(payload: any) => void} [onCullFinished]
 * @property {(payload: any) => void} [onCullFailed]
 * @property {(progress: any) => void} [onExportProgress]
 * @property {(progress: any) => void} [onPublishProgress]
 * @property {import('./devPanelBridge.js').DevPanelBridgeHandlers} [devPanel]
 * @property {import('./settingsPanelBridge.js').SettingsPanelBridgeHandlers} [settingsPanel]
 * @property {import('./menuBridge.js').MenuBridgeHandlers} [menu]
 */

/**
 * Registers all core backend event listeners for the Reveal main window.
 * @param {(event: string, handler: (e: { payload: any }) => void) => void} on
 * @param {AppEventListenerHandlers} handlers
 */
export function registerAppEventListeners(on, handlers) {
  // Shell and System events
  on("garden-account-changed", (e) => handlers.onGardenAccountChanged?.(e.payload));
  on("focus-mode-changed", (e) => handlers.onFocusModeChanged?.(e.payload));
  on("shell-prefs-changed", (e) => handlers.onShellPrefsChanged?.(e.payload));
  on("app-error", (e) => handlers.onAppError?.(e.payload));
  on("libraries-changed", () => handlers.onLibrariesChanged?.());
  on("source-offline", () => handlers.onSourceOffline?.());

  // Indexing and Scanning
  on("index-progress", (e) => handlers.onIndexProgress?.(e.payload));

  // Memory card detection and management
  on("cards-changed", (e) => handlers.onCardsChanged?.(e.payload));
  on("card-mounted", (e) => handlers.onCardMounted?.(e.payload));
  on("card-unmounted", (e) => handlers.onCardUnmounted?.(e.payload));
  on("import-first-card-requested", () => handlers.onImportFirstCardRequested?.());

  // Import lifecycle
  on("import-progress", (e) => handlers.onImportProgress?.(e.payload));
  on("import-preview-ready", (e) => handlers.onImportPreviewReady?.(e.payload));
  on("import-started", (e) => handlers.onImportStarted?.(e.payload));
  on("import-finished", (e) => handlers.onImportFinished?.(e.payload));
  on("import-failed", (e) => handlers.onImportFailed?.(e.payload));

  // AI Culling lifecycle
  on("cull-started", (e) => handlers.onCullStarted?.(e.payload));
  on("cull-progress", (e) => handlers.onCullProgress?.(e.payload));
  on("cull-finished", (e) => handlers.onCullFinished?.(e.payload));
  on("cull-failed", (e) => handlers.onCullFailed?.(e.payload));

  // Export & Publication
  on("export-progress", (e) => handlers.onExportProgress?.(e.payload));
  on("publish-progress", (e) => handlers.onPublishProgress?.(e.payload));

  // Detached panels & Menu bridges
  if (handlers.devPanel) {
    registerDevPanelBridge(on, handlers.devPanel);
  }
  if (handlers.settingsPanel) {
    registerSettingsPanelBridge(on, handlers.settingsPanel);
  }
  if (handlers.menu) {
    registerMenuBridge(on, handlers.menu);
  }
}
