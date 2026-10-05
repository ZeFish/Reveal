/**
 * Bridge for IPC communication between the main window and the Settings Panel window.
 *
 * @typedef {Object} SettingsPanelBridgeHandlers
 * @property {() => void} [onReady]
 * @property {(key: string) => void} [onChooseFolder]
 * @property {(preferences: any) => void} [onSave]
 * @property {() => void} [onOpenSettings]
 */

/**
 * Registers all event listeners for the Settings Panel.
 * @param {(event: string, handler: (e: { payload: any }) => void) => void} on - Register function that tracks unlisteners.
 * @param {SettingsPanelBridgeHandlers} handlers
 */
export function registerSettingsPanelBridge(on, handlers) {
  on("settings-panel-ready", () => handlers.onReady?.());
  on("settings-panel-choose-folder", (e) => {
    handlers.onChooseFolder?.(e.payload?.key);
  });
  on("settings-panel-save", (e) => {
    handlers.onSave?.(e.payload?.preferences);
  });
  on("open-settings-requested", () => handlers.onOpenSettings?.());
}
