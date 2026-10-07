/**
 * Keyboard shortcut controller.
 *
 * Coordinates global keydown dispatching, workflow action execution (mode switching,
 * space look, preview filter toggle), navigation, and selection manipulation.
 */

import { dispatchShortcut, isCompareKey } from "./shortcutDispatcher.js";

/**
 * Creates a bound keyboard shortcut controller.
 *
 * @param {{
 *   getState: () => {
 *     currentMode: "cull" | "dev",
 *     fullscreen: boolean,
 *     spaceLook: boolean,
 *     previewFilter: boolean,
 *     devPanel: boolean,
 *     view: any[],
 *     sel: number,
 *     photoPath: string | null,
 *     recipe: any,
 *     lastEditedKey: string | null,
 *     developEngine: string,
 *     developPhotoPercent: number,
 *     cols: number,
 *     marginScale: number,
 *     zoneMaskPreview: any,
 *     dockedActiveZone: string,
 *     selectionAnchor: number,
 *   },
 *   actions: {
 *     switchMode: (mode: "cull" | "dev" | any, opts?: any) => Promise<any> | void,
 *     setSpaceLook?: (v: boolean) => void,
 *     setDevPanel?: (v: boolean) => void,
 *     saveLayouts: () => void,
 *     toggleSidebar: () => void,
 *     exitFullscreen: () => void,
 *     toggleFullscreen: () => void,
 *     togglePreviewFilter: (val?: boolean) => void,
 *     toggleFocusMode: () => void,
 *     toggleAppearance: () => void,
 *     toggleLayout: () => void,
 *     toggleStory: () => void,
 *     compareDown?: () => void,
 *     compareUp?: () => void,
 *     toggleShortcuts: () => void,
 *     toggleDevPanel: () => void,
 *     cycleZoom: (reverse?: boolean) => void,
 *     selectAll: (view: any[]) => void,
 *     copyImageToClipboard: (targetPath: string) => void,
 *     clearSelection: () => void,
 *     undoRecipeEdit: () => void,
 *     redoRecipeEdit: () => void,
 *     exportSelection: () => void,
 *     copySettings: () => void,
 *     pasteSettings: () => void,
 *     setZoneMask: (mask: any) => void,
 *     setPhotoSize: (size: number) => void,
 *     setCols: (n: number) => void,
 *     setMarginScale: (m: number) => void,
 *     updateRecipe: (newRecipe: any) => void,
 *     focusAt: (view: any[], nextSel: number) => void,
 *     selectRange: (view: any[], anchor: number, sel: number) => void,
 *     selectOnly: (view: any[], sel: number) => void,
 *     prepareFullscreenFrame: (path: string) => void,
 *     openPhoto: (path: string) => void,
 *     rate: (n: number) => void,
 *     toggleSelected: (path: string) => void,
 *     setAnchor: (path: string) => void,
 *   },
 * }} deps
 */
export function createKeyboardController(deps) {
  const { getState, actions } = deps;

  /**
   * @param {any} res
   * @returns {boolean}
   */
  function applyWorkflowResult(res) {
    if (!res || res.action === "NONE") return false;
    const currentState = getState();

    if (res.action === "SWITCH_MODE") {
      actions.switchMode(res.to, { openDevPanel: res.openDevPanel ?? true });
      if (res.spaceLook !== undefined && actions.setSpaceLook) {
        actions.setSpaceLook(res.spaceLook);
      }
      return true;
    }
    if (res.action === "OPEN_DEV_PANEL") {
      if (actions.setSpaceLook) actions.setSpaceLook(res.spaceLook ?? false);
      actions.setDevPanel?.(true);
      actions.saveLayouts();
      if (currentState.currentMode !== "dev") {
        actions.switchMode("dev", { openDevPanel: true });
      }
      return true;
    }
    if (res.action === "TOGGLE_SIDEBAR") {
      actions.toggleSidebar();
      return true;
    }
    if (res.action === "EXIT_FULLSCREEN") {
      actions.exitFullscreen();
      return true;
    }
    if (res.action === "TOGGLE_PREVIEW") {
      if (res.andSwitchToCull) {
        if (currentState.currentMode !== "cull") actions.switchMode("cull", { openDevPanel: false });
        actions.togglePreviewFilter(true);
      } else {
        actions.togglePreviewFilter();
      }
      return true;
    }
    if (res.action === "GO_TO_GRID") {
      actions.togglePreviewFilter(false);
      if (currentState.currentMode !== "cull") actions.switchMode("cull", { openDevPanel: false });
      return true;
    }
    return false;
  }

  /** @param {KeyboardEvent} e */
  function onKey(e) {
    const state = getState();
    dispatchShortcut(e, state, {
      selectAll: () => actions.selectAll(state.view),
      copyImage: (targetPath) => actions.copyImageToClipboard(targetPath),
      clearSelection: () => actions.clearSelection(),
      undo: () => actions.undoRecipeEdit(),
      redo: () => actions.redoRecipeEdit(),
      toggleFullscreen: () => actions.toggleFullscreen(),
      exitFullscreen: () => actions.exitFullscreen(),
      toggleFocus: () => actions.toggleFocusMode(),
      toggleAppearance: () => actions.toggleAppearance(),
      toggleSidebar: () => actions.toggleSidebar(),
      toggleLayout: () => actions.toggleLayout(),
      setZoneMask: (mask) => actions.setZoneMask(mask),
      toggleStory: () => actions.toggleStory(),
      compareDown: () => actions.compareDown?.(),
      toggleShortcuts: () => actions.toggleShortcuts(),
      exportSelection: () => actions.exportSelection(),
      copySettings: () => actions.copySettings(),
      pasteSettings: () => actions.pasteSettings(),
      toggleDevPanel: () => actions.toggleDevPanel(),
      cycleZoom: (reverse) => actions.cycleZoom(reverse),
      applyWorkflowResult,
      setPhotoSize: (size) => actions.setPhotoSize(size),
      setCols: (n) => actions.setCols(n),
      setMarginScale: (m) => actions.setMarginScale(m),
      updateRecipe: (newRecipe) => actions.updateRecipe(newRecipe),
      navigate: (nextSel, { shiftKey, metaKey }) => {
        actions.focusAt(state.view, nextSel);
        if (shiftKey) {
          actions.selectRange(state.view, state.selectionAnchor, state.sel);
        } else if (metaKey) {
          // macOS/Windows pattern: Cmd/Ctrl + Arrow moves cursor without changing selection
        } else {
          actions.selectOnly(state.view, state.sel);
        }
        if (state.fullscreen) {
          actions.prepareFullscreenFrame(state.view[state.sel]?.path);
        } else if (state.currentMode === "dev" && state.view[state.sel]) {
          actions.openPhoto(state.view[state.sel]?.path);
        }
        if (typeof document !== "undefined") {
          document.querySelector(`[data-idx="${state.sel}"]`)?.scrollIntoView({ block: "nearest" });
        }
      },
      rate: (n) => actions.rate(n),
      toggleSelected: (path) => {
        actions.toggleSelected(path);
        actions.setAnchor(path);
      },
    });
  }

  /**
   * A key comes up. Only the before / after key cares: letting go ends a hold.
   * @param {KeyboardEvent} e
   */
  function onKeyUp(e) {
    if (isCompareKey(e)) actions.compareUp?.();
  }

  return {
    onKey,
    onKeyUp,
    applyWorkflowResult,
  };
}
