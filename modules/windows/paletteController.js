/**
 * Palette Windows Controller.
 *
 * Coordinates detached develop palettes, multi-window state synchronization,
 * and visibility toggling.
 */

import { syncPalettes, syncDevPanelWindow } from "./paletteManager.js";

/**
 * Creates a bound palette controller instance.
 *
 * @param {{
 *   isTauri: boolean,
 *   emit?: (event: string, payload?: any) => Promise<any> | void,
 *   getCurrentMode: () => "cull" | "dev",
 *   getLayouts: () => any,
 *   getSpaceLook: () => boolean,
 *   getPicked: () => string | null,
 *   getPhotoPath: () => string | null,
 *   getDevelopState: () => any,
 *   getExportState: () => any,
 *   getFrames: () => any[],
 *   getInstalledEditors: () => [string, string][],
 *   getStatus: () => string,
 *   saveLayouts: () => void,
 *   switchMode: (mode: "cull" | "dev", opts?: any) => Promise<any> | void,
 *   setSpaceLook: (val: boolean) => void,
 * }} deps
 */
export function createPaletteController(deps) {
  const {
    isTauri,
    emit,
    getCurrentMode,
    getLayouts,
    getSpaceLook,
    getPicked,
    getPhotoPath,
    getDevelopState,
    getExportState,
    getFrames,
    getInstalledEditors,
    getStatus,
    saveLayouts,
    switchMode,
    setSpaceLook,
  } = deps;

  function sendDevStateToPanel() {
    if (!isTauri || getCurrentMode() !== "dev" || !emit) return;
    const photoPath = getPhotoPath();
    const developState = getDevelopState();
    const exportState = getExportState();
    const frames = getFrames();
    const picked = getPicked();
    const status = getStatus();
    const installedEditors = getInstalledEditors();

    emit("main-dev-state", {
      photoPath,
      picked,
      rating: frames.find((f) => f.path === photoPath)?.rating ?? 0,
      recipe: developState.recipe ? { ...developState.recipe } : null,
      developEngine: developState.developEngine,
      renderMs: developState.renderMs,
      status,
      installedEditors,
      exportEdge: exportState?.edge,
      exportBorder: exportState?.border,
      exportFolder: exportState?.folder,
      films: developState.films,
      papers: developState.papers,
      luts: developState.luts,
      engines: developState.engines,
      caption: developState.caption,
      tags: developState.tags,
      showClipping: developState.showClipping,
      checkLayer: developState.checkLayer,
      showCaption: developState.showCaption,
      photoScale: developState.developPhotoPercent,
      // Typed arrays don't survive Tauri's JSON emit as themselves — plain
      // arrays round-trip fine and index identically in Histogram.svelte.
      histogram: developState.histogram
        ? {
            r: Array.from(developState.histogram.r),
            g: Array.from(developState.histogram.g),
            b: Array.from(developState.histogram.b),
            luma: Array.from(developState.histogram.luma),
          }
        : null,
    })?.catch?.(() => {});
  }

  function handleSyncPalettes() {
    syncPalettes({
      currentMode: getCurrentMode(),
      layouts: getLayouts(),
      spaceLook: getSpaceLook(),
      picked: getPicked(),
      sendDevStateToPanel,
      saveLayouts,
    });
  }

  function handleSyncDevPanelWindow() {
    return syncDevPanelWindow({
      currentMode: getCurrentMode(),
      layouts: getLayouts(),
      spaceLook: getSpaceLook(),
      picked: getPicked(),
      sendDevStateToPanel,
      saveLayouts,
    });
  }

  function toggleDevPanel() {
    const spaceLook = getSpaceLook();
    const layouts = getLayouts();
    const mode = getCurrentMode();

    if (spaceLook && layouts.dev?.devPanel) {
      setSpaceLook(false);
      if (mode !== "dev") switchMode("dev", { openDevPanel: false });
      return;
    }
    if (mode !== "dev") {
      switchMode("dev", { openDevPanel: true });
      return;
    }
    setSpaceLook(false);
    if (layouts.dev) {
      layouts.dev.devPanel = !layouts.dev.devPanel;
      saveLayouts();
    }
  }

  return {
    sendDevStateToPanel,
    handleSyncPalettes,
    handleSyncDevPanelWindow,
    toggleDevPanel,
  };
}
