/**
 * Bridge for IPC communication between the main window and the detached Dev Panel.
 *
 * @typedef {Object} DevPanelBridgeHandlers
 * @property {() => void} [onReady]
 * @property {(payload: { key?: string, recipe?: any, transient?: boolean }) => void} [onRecipeUpdated]
 * @property {(engine: string) => void} [onEngineUpdated]
 * @property {(caption: string) => void} [onCaptionUpdated]
 * @property {(tags: string[]) => void} [onTagsUpdated]
 * @property {() => void} [onExport]
 * @property {() => void} [onExportDesktop]
 * @property {() => void} [onExportVault]
 * @property {() => void} [onReset]
 * @property {(settings: { exportEdge: number, exportBorder: boolean }) => void} [onExportSettingsChanged]
 * @property {(scale: number) => void} [onPhotoScaleChanged]
 * @property {() => void} [onChooseExportFolder]
 * @property {(appPath: string) => void} [onOpenInEditor]
 * @property {(mode: "cull" | "dev") => void} [onSwitchMode]
 * @property {(keyEvent: { key: string, metaKey?: boolean, ctrlKey?: boolean, shiftKey?: boolean }) => void} [onForwardKey]
 * @property {(payload?: { reverse?: boolean }) => void} [onZoom]
 * @property {() => void} [onClose]
 * @property {() => void} [onDockRequested]
 * @property {(payload: { checkLayer?: string }) => void} [onSetCheckLayer]
 * @property {(payload: { checkLayer?: string, showClipping?: boolean }) => void} [onToggleClipping]
 * @property {(payload: { showCaption?: boolean }) => void} [onToggleCaption]
 * @property {(zoneMask: "highlights" | "shadows" | "midtones" | null) => void} [onSetZoneMask]
 * @property {(payload: { recipe?: any }) => void} [onPresetPreview]
 * @property {(payload: { recipe?: any, scope?: string }) => void} [onPresetApply]
 * @property {() => void} [onTogglePresetPanel]
 * @property {() => void} [onToggleLutPanel]
 * @property {() => void} [onToggleDevPanel]
 */

/**
 * Registers all event listeners for the detached dev panel and preset palettes.
 * @param {(event: string, handler: (e: { payload: any }) => void) => void} on - Register function that tracks unlisteners.
 * @param {DevPanelBridgeHandlers} handlers
 */
export function registerDevPanelBridge(on, handlers) {
  on("dev-panel-ready", () => handlers.onReady?.());
  on("dev-panel-recipe-updated", (e) => handlers.onRecipeUpdated?.(e.payload));
  on("dev-panel-engine-updated", (e) => handlers.onEngineUpdated?.(e.payload?.engine));
  on("dev-panel-caption-updated", (e) => handlers.onCaptionUpdated?.(e.payload?.caption));
  on("dev-panel-tags-updated", (e) => handlers.onTagsUpdated?.(e.payload?.tags));
  on("dev-panel-export", () => handlers.onExport?.());
  on("dev-panel-export-desktop", () => handlers.onExportDesktop?.());
  on("dev-panel-export-vault", () => handlers.onExportVault?.());
  on("dev-panel-export-daily", () => handlers.onExportVault?.());
  on("dev-panel-reset", () => handlers.onReset?.());
  on("dev-panel-export-settings-changed", (e) => handlers.onExportSettingsChanged?.(e.payload));
  on("dev-panel-photo-scale-changed", (e) => handlers.onPhotoScaleChanged?.(e.payload?.photoScale));
  on("dev-panel-choose-export-folder", () => handlers.onChooseExportFolder?.());
  on("dev-panel-open-in-editor", (e) => handlers.onOpenInEditor?.(e.payload?.appPath));
  on("dev-panel-switch-mode", (e) => handlers.onSwitchMode?.(e.payload?.mode));
  on("dev-panel-key", (e) => handlers.onForwardKey?.(e.payload));
  on("dev-panel-zoom", (e) => handlers.onZoom?.(e.payload));
  on("dev-panel-close", () => handlers.onClose?.());
  on("dev-panel-dock-requested", () => handlers.onDockRequested?.());
  on("dev-panel-set-check-layer", (e) => handlers.onSetCheckLayer?.(e.payload));
  on("dev-panel-toggle-clipping", (e) => handlers.onToggleClipping?.(e.payload));
  on("dev-panel-toggle-caption", (e) => handlers.onToggleCaption?.(e.payload));
  on("dev-panel-set-zone-mask", (e) => handlers.onSetZoneMask?.(e.payload?.zoneMask ?? null));

  on("preset-preview", (e) => handlers.onPresetPreview?.(e.payload));
  on("preset-apply", (e) => handlers.onPresetApply?.(e.payload));
  on("toggle-preset-panel-requested", () => handlers.onTogglePresetPanel?.());
  on("toggle-lut-panel-requested", () => handlers.onToggleLutPanel?.());
  on("toggle-dev-panel-requested", () => handlers.onToggleDevPanel?.());
}
