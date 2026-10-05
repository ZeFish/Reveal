/**
 * Bridge for native macOS menu items, dock events, and system requests.
 *
 * @typedef {Object} MenuBridgeHandlers
 * @property {() => void} [onToggleRenderQueue]
 * @property {() => void} [onToggleShortcuts]
 * @property {() => void} [onMenuExport]
 * @property {() => void} [onMenuResetDevelop]
 * @property {() => void} [onMenuClearDevelop]
 * @property {() => void} [onMenuEnableDevelop]
 * @property {() => void} [onMenuPublish]
 * @property {() => void} [onDockReopen]
 * @property {(path: string) => void} [onOpenFile]
 * @property {() => void} [onToggleAutoImport]
 * @property {() => void} [onAddLibraryFolder]
 */

/**
 * Registers all event listeners for native menus and macOS shell requests.
 * @param {(event: string, handler: (e: { payload: any }) => void) => void} on - Register function that tracks unlisteners.
 * @param {MenuBridgeHandlers} handlers
 */
export function registerMenuBridge(on, handlers) {
  on("toggle-render-queue-requested", () => handlers.onToggleRenderQueue?.());
  on("toggle-shortcuts-requested", () => handlers.onToggleShortcuts?.());
  on("menu-export-requested", () => handlers.onMenuExport?.());
  on("menu-reset-develop-requested", () => handlers.onMenuResetDevelop?.());
  on("menu-clear-develop-requested", () => handlers.onMenuClearDevelop?.());
  on("menu-enable-develop-requested", () => handlers.onMenuEnableDevelop?.());
  on("menu-publish-requested", () => handlers.onMenuPublish?.());
  on("dock-reopen-requested", () => handlers.onDockReopen?.());
  on("open-file-requested", (e) => handlers.onOpenFile?.(e.payload));
  on("toggle-auto-import-requested", () => handlers.onToggleAutoImport?.());
  on("add-library-folder-requested", () => handlers.onAddLibraryFolder?.());
}
