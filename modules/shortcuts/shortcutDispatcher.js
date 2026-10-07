import { ModeNavigation } from "./modeNavigation.js";
import { SETTING_STEPS } from "./settingSteps.js";
import { DEFAULT_PHOTO_SIZE, PHOTO_SIZE_MIN, PHOTO_SIZE_MAX } from "@modules/core";

/**
 * Checks whether the current keystroke should be ignored because
 * focus is in a text input, open dialog, or menu.
 * @param {KeyboardEvent} e
 * @returns {boolean}
 */
export function shouldIgnoreKeystroke(e) {
  if (e.defaultPrevented) return true;
  if (typeof document !== "undefined") {
    if (document.querySelector("dialog[open]")) return true;
    if (
      e.target instanceof Element &&
      e.target.closest("[role='menu'], [role='dialog'], [contenteditable='true']")
    ) {
      return true;
    }
  }
  const targetTag = /** @type {HTMLElement} */ (e.target)?.tagName;
  if (targetTag && ["INPUT", "SELECT", "TEXTAREA"].includes(targetTag)) return true;

  if (["Enter", " "].includes(e.key) && e.target instanceof Element) {
    const owner = e.target.closest("button, [role='button']:not(.cell)");
    if (owner?.matches(":focus-visible")) return true;
  }

  return false;
}

/**
 * @typedef {Object} ShortcutState
 * @property {"cull" | "dev"} currentMode
 * @property {boolean} fullscreen
 * @property {boolean} spaceLook
 * @property {boolean} previewFilter
 * @property {boolean} devPanel
 * @property {any[]} view
 * @property {number} sel
 * @property {string | null} photoPath
 * @property {any} recipe
 * @property {string | null} lastEditedKey
 * @property {string} developEngine
 * @property {number} developPhotoPercent
 * @property {number} cols
 * @property {number} marginScale
 * @property {"highlights" | "shadows" | "midtones" | null} zoneMaskPreview
 * @property {string} dockedActiveZone
 * @property {number | null} selectionAnchor
 *
 * @typedef {Object} ShortcutActions
 * @property {() => void} [selectAll]
 * @property {(path: string) => void} [copyImage]
 * @property {() => void} [clearSelection]
 * @property {() => void} [undo]
 * @property {() => void} [redo]
 * @property {() => void} [toggleFullscreen]
 * @property {() => void} [exitFullscreen]
 * @property {() => void} [toggleFocus]
 * @property {() => void} [toggleAppearance]
 * @property {() => void} [toggleSidebar]
 * @property {() => void} [toggleLayout]
 * @property {(mask: "highlights" | "shadows" | "midtones" | null) => void} [setZoneMask]
 * @property {() => void} [toggleStory]
 * @property {() => void} [compareDown]
 * @property {() => void} [toggleShortcuts]
 * @property {() => void} [exportSelection]
 * @property {() => void} [copySettings]
 * @property {() => void} [pasteSettings]
 * @property {() => void} [toggleDevPanel]
 * @property {(reverse?: boolean) => void} [cycleZoom]
 * @property {(res: any) => void} [applyWorkflowResult]
 * @property {(size: number) => void} [setPhotoSize]
 * @property {(cols: number) => void} [setCols]
 * @property {(scale: number) => void} [setMarginScale]
 * @property {(newRecipe: any) => void} [updateRecipe]
 * @property {(nextSel: number, opts: { shiftKey: boolean, metaKey: boolean }) => void} [navigate]
 * @property {(rating: number) => void} [rate]
 * @property {(path: string) => void} [toggleSelected]
 */

/**
 * The before / after key: backslash, as in Lightroom, or Y (backslash is a chord on some keyboards).
 * @param {{ key: string }} e
 * @returns {boolean}
 */
export function isCompareKey(e) {
  return e.key === "\\" || e.key.toLowerCase() === "y";
}

/**
 * Dispatches a keyboard event based on current application mode and state.
 * Returns true if the key was handled, false otherwise.
 * @param {KeyboardEvent} e
 * @param {ShortcutState} state
 * @param {ShortcutActions} actions
 * @returns {boolean}
 */
export function dispatchShortcut(e, state, actions) {
  if (shouldIgnoreKeystroke(e)) return false;

  const controller = new ModeNavigation({
    currentMode: state.currentMode,
    spaceLook: state.spaceLook,
    devPanel: state.devPanel,
  });

  // ⌘A / Ctrl-A — select all
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
    actions.selectAll?.();
    e.preventDefault();
    return true;
  }

  // ⌘C / Ctrl-C — copy photo image to OS clipboard
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "c") {
    const targetPath = controller.resolveCopyTarget({
      photoPath: state.photoPath,
      selectedFramePath: state.view[state.sel]?.path,
    });
    if (targetPath) {
      actions.copyImage?.(targetPath);
      e.preventDefault();
      return true;
    }
  }

  // ⌘D / Ctrl-D — deselect all
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "d") {
    actions.clearSelection?.();
    e.preventDefault();
    return true;
  }

  // ⌘Z — undo / redo recipe edit
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
    if (e.shiftKey) actions.redo?.();
    else actions.undo?.();
    e.preventDefault();
    return true;
  }

  // F — Fullscreen
  if (e.key.toLowerCase() === "f") {
    actions.toggleFullscreen?.();
    e.preventDefault();
    return true;
  }

  // Escape / Space / G while fullscreen -> exit fullscreen
  if (state.fullscreen && (e.key === "Escape" || e.key === " " || e.key.toLowerCase() === "g")) {
    actions.exitFullscreen?.();
    e.preventDefault();
    return true;
  }

  // Global layout toggles
  if (e.key === "o") {
    actions.toggleFocus?.();
    e.preventDefault();
    return true;
  }
  if (e.key === "l") {
    actions.toggleAppearance?.();
    e.preventDefault();
    return true;
  }
  if (e.key === "b") {
    actions.toggleSidebar?.();
    e.preventDefault();
    return true;
  }

  // \ or Y — before / after in Develop (Lightroom's keys): a tap switches, a hold peeks and
  // comes back on release. The release is handled by `isCompareKey` on keyup.
  if (isCompareKey(e) && state.currentMode === "dev" && !e.metaKey && !e.ctrlKey) {
    if (!e.repeat) actions.compareDown?.();
    e.preventDefault();
    return true;
  }

  // M — zone mask in Dev mode, masonry in Cull mode
  if (e.key === "m") {
    if (state.currentMode === "dev") {
      /** @type {"highlights" | "shadows" | "midtones" | null} */
      const newMask = state.zoneMaskPreview
        ? null
        : /** @type {any} */ (state.dockedActiveZone === "global" ? "shadows" : state.dockedActiveZone);
      actions.setZoneMask?.(newMask);
    } else {
      actions.toggleLayout?.();
    }
    e.preventDefault();
    return true;
  }

  // Q — Toggle selected photo in story note
  if (e.key === "q") {
    actions.toggleStory?.();
    e.preventDefault();
    return true;
  }

  // ⌘0 / Ctrl-0 / ⌘à — Reset photo size in Develop mode
  if ((e.metaKey || e.ctrlKey) && (e.key === "0" || e.key === "à")) {
    if (state.currentMode === "dev") {
      actions.setPhotoSize?.(DEFAULT_PHOTO_SIZE);
      e.preventDefault();
      return true;
    }
  }

  // Zoom shortcuts (+ / - / = / _)
  if (e.key === "=" || e.key === "+" || e.key === "-" || e.key === "_") {
    if (state.currentMode === "dev" && !e.ctrlKey) {
      if (e.key === "=" || e.key === "+") {
        actions.setPhotoSize?.(Math.min(PHOTO_SIZE_MAX, state.developPhotoPercent + 5));
        e.preventDefault();
        return true;
      }
      if (e.key === "-" || e.key === "_") {
        actions.setPhotoSize?.(Math.max(PHOTO_SIZE_MIN, state.developPhotoPercent - 5));
        e.preventDefault();
        return true;
      }
    } else if (!e.metaKey && !e.ctrlKey) {
      if (e.key === "=") {
        actions.setCols?.(Math.max(1, state.cols - 1));
        e.preventDefault();
        return true;
      }
      if (e.key === "+") {
        actions.setMarginScale?.(Math.min(6, Math.round((state.marginScale + 0.25) * 100) / 100));
        e.preventDefault();
        return true;
      }
      if (e.key === "-") {
        actions.setCols?.(Math.min(12, state.cols + 1));
        e.preventDefault();
        return true;
      }
      if (e.key === "_") {
        actions.setMarginScale?.(Math.max(0.25, Math.round((state.marginScale - 0.25) * 100) / 100));
        e.preventDefault();
        return true;
      }
    }
  }

  // Copy / Paste settings (c / v without meta/ctrl)
  if (!e.metaKey && !e.ctrlKey && e.key.toLowerCase() === "c") {
    actions.copySettings?.();
    e.preventDefault();
    return true;
  }
  if (!e.metaKey && !e.ctrlKey && e.key.toLowerCase() === "v") {
    actions.pasteSettings?.();
    e.preventDefault();
    return true;
  }

  // ⇧D — Toggle develop panel
  if (e.key.toLowerCase() === "d" && e.shiftKey) {
    actions.toggleDevPanel?.();
    e.preventDefault();
    return true;
  }

  // Z — Cycle zoom
  if (e.key.toLowerCase() === "z" && !e.metaKey && !e.ctrlKey && controller.canCycleZoom()) {
    actions.cycleZoom?.(e.shiftKey);
    e.preventDefault();
    return true;
  }

  // ? / H — Shortcuts help
  if (e.key === "?" || e.key === "h") {
    actions.toggleShortcuts?.();
    e.preventDefault();
    return true;
  }

  // R — Reveal / Export selection
  if (e.key === "r" && !e.metaKey && !e.ctrlKey) {
    actions.exportSelection?.();
    e.preventDefault();
    return true;
  }

  // Mode switcher shortcuts handled via AppController
  if (e.key === "g") {
    actions.applyWorkflowResult?.(controller.handleG({ previewFilter: state.previewFilter }));
    e.preventDefault();
    return true;
  }
  if (e.key === "d" && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
    actions.applyWorkflowResult?.(controller.handleD());
    e.preventDefault();
    return true;
  }
  if (e.key === "s") {
    actions.applyWorkflowResult?.(controller.handleS());
    e.preventDefault();
    return true;
  }
  if (e.key === "Escape") {
    actions.applyWorkflowResult?.(
      controller.handleEscape({
        hasOverlay: false,
        fullscreen: state.fullscreen,
        previewFilter: state.previewFilter,
      })
    );
    e.preventDefault();
    return true;
  }

  // In Develop mode: ArrowUp / ArrowDown modifies the last edited setting
  if (
    controller.canAdjustRecipe() &&
    (e.key === "ArrowUp" || e.key === "ArrowDown") &&
    !e.metaKey &&
    !e.ctrlKey
  ) {
    if (state.recipe && state.lastEditedKey) {
      const config = SETTING_STEPS[state.lastEditedKey] || { step: 0.05, shiftStep: 0.25 };
      const step = e.shiftKey ? config.shiftStep : config.step;
      const currentVal = Number(state.recipe[state.lastEditedKey] ?? 0);
      const delta = e.key === "ArrowUp" ? step : -step;
      const newVal = Math.round((currentVal + delta) * 1000) / 1000;

      const newRecipe = { ...state.recipe, [state.lastEditedKey]: newVal };
      if (!state.developEngine || state.developEngine === "none") newRecipe.engine = "spektra";
      else newRecipe.engine = state.developEngine;

      actions.updateRecipe?.(newRecipe);
      e.preventDefault();
      return true;
    }
  }

  // Navigation: Arrow keys
  const isNav = ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key);
  if (isNav) {
    const c = controller.navColumnCount({ fullscreen: state.fullscreen, cols: state.cols });
    let nextSel = state.sel;
    if (e.key === "ArrowRight") nextSel = Math.min(state.sel + 1, state.view.length - 1);
    else if (e.key === "ArrowLeft") nextSel = Math.max(state.sel - 1, 0);
    else if (e.key === "ArrowDown") nextSel = Math.min(state.sel + c, state.view.length - 1);
    else if (e.key === "ArrowUp") nextSel = Math.max(state.sel - c, 0);

    actions.navigate?.(nextSel, {
      shiftKey: !!e.shiftKey,
      metaKey: !!(e.metaKey || e.ctrlKey),
    });
    e.preventDefault();
    return true;
  }

  // Space
  if (e.key === " " && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
    if (controller.isCull()) {
      if (!state.view[state.sel]?.path) {
        if (!state.view.length) return false;
        actions.navigate?.(0, { shiftKey: false, metaKey: false });
      }
      if (!state.view[state.sel]?.path && !state.view[0]?.path) return false;
    }
    actions.applyWorkflowResult?.(controller.handleSpace());
    e.preventDefault();
    return true;
  }

  // Cmd-Space — Toggle selection
  if (e.key === " " && (e.metaKey || e.ctrlKey)) {
    if (state.view[state.sel]) {
      actions.toggleSelected?.(state.view[state.sel].path);
    }
    e.preventDefault();
    return true;
  }

  // 0..5 — Rating
  if (e.key >= "0" && e.key <= "5") {
    actions.rate?.(Number(e.key));
    e.preventDefault();
    return true;
  }

  return false;
}
