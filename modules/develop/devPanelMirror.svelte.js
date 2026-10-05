import { onMount } from "svelte";
import { DEFAULT_PHOTO_SIZE } from "@modules/core";
import { listen, emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@modules/core";

/** @typedef {{ name: string, label: string }} FilmOrPaper */
/** @typedef {{ id: string, label: string, control_groups?: any[] }} EngineInfo */
/** @typedef {Record<string, any>} Recipe */

/**
 * Controller for the detached Develop window host.
 *
 * Synchronizes local mirror state with backend Tauri commands and the main window.
 */
export function createDevPanelMirror() {
  /** @type {string | null} */
  let photoPath = $state(null);
  /** @type {string | null} */
  let picked = $state(null);
  /** @type {Recipe | null} */
  let recipe = $state(null);
  /** @type {string | null} */
  let developEngine = $state(null);
  /** @type {number | null} */
  let renderMs = $state(null);
  let status = $state("");
  /** @type {[string, string][]} */
  let installedEditors = $state([]);
  let exportEdge = $state(2048);
  let exportBorder = $state(false);
  let exportFolder = $state("");
  /** @type {FilmOrPaper[]} */
  let films = $state([]);
  /** @type {FilmOrPaper[]} */
  let papers = $state([]);
  /** @type {(string | {name: string, opacity: number})[]} */
  let luts = $state([]);
  /** @type {EngineInfo[]} */
  let engines = $state([]);
  let caption = $state("");
  /** @type {string[]} */
  let tags = $state([]);
  let rating = $state(0);
  let publishing = $state(false);
  let publishStatus = $state("");
  /** @type {Recipe | null} */
  let defaults = $state(null);
  let showClipping = $state(false);
  let checkLayer = $state("none");
  let lastActiveCheckLayer = "clipping";
  let showCaption = $state(false);
  /** @type {{r: number[], g: number[], b: number[], luma: number[]} | null} */
  let histogram = $state(null);
  let photoScale = $state(DEFAULT_PHOTO_SIZE);

  function photoScaleChanged() {
    emit("dev-panel-photo-scale-changed", { photoScale });
  }

  /** @param {string | null} mode */
  function toggleCheckLayer(mode = null) {
    if (mode) {
      checkLayer = checkLayer === mode ? "none" : mode;
      if (checkLayer !== "none") lastActiveCheckLayer = checkLayer;
    } else {
      if (checkLayer !== "none") {
        checkLayer = "none";
      } else {
        checkLayer = lastActiveCheckLayer || "clipping";
      }
    }
    showClipping = checkLayer !== "none";
    emit("dev-panel-set-check-layer", { checkLayer });
  }

  /** @param {string | null} mode */
  function toggleClipping(mode = null) {
    toggleCheckLayer(mode);
  }

  function toggleCaptionOverlay() {
    showCaption = !showCaption;
    emit("dev-panel-toggle-caption", { showCaption });
  }

  function captionEdited() {
    emit("dev-panel-caption-updated", { caption });
  }

  function tagsEdited() {
    emit("dev-panel-tags-updated", { tags });
  }

  /** @param {string} stage */
  const lutsKey = (stage) => (stage === "pre" ? "rapid_pre_luts" : "rapid_post_luts");
  /** @param {string} stage */
  const oldLutsKey = (stage) => (stage === "pre" ? "pre_luts" : "post_luts");

  /** @param {Recipe} r @param {string} stage */
  function ensureMigration(r, stage) {
    if (!r || developEngine !== "rapid") return;
    const key = lutsKey(stage);
    const oldKey = oldLutsKey(stage);
    if (r[oldKey]?.length > 0) {
      r[key] = [...(r[key] ?? []), ...r[oldKey]];
      r[oldKey] = [];
      edited();
    }
  }

  /** @param {string} stage */
  function addLutLayer(stage) {
    if (!recipe) return;
    ensureMigration(recipe, stage);
    const key = lutsKey(stage);
    const first = typeof luts[0] === "string" ? luts[0] : (luts[0]?.name ?? "");
    recipe[key] = [...(recipe[key] ?? []), { name: first, opacity: 1 }];
    edited();
  }

  /** @param {string} stage @param {number} index */
  function removeLutLayer(stage, index) {
    if (!recipe) return;
    ensureMigration(recipe, stage);
    const key = lutsKey(stage);
    recipe[key] = (recipe[key] ?? []).filter((/** @type {any} */ _, /** @type {number} */ i) => i !== index);
    edited();
  }

  /** @param {string} stage @param {number} index @param {number | string} value */
  function updateLutOpacity(stage, index, value) {
    if (!recipe) return;
    ensureMigration(recipe, stage);
    const key = lutsKey(stage);
    recipe[key] = (recipe[key] ?? []).map((/** @type {any} */ l, /** @type {number} */ i) => (i === index ? { ...l, opacity: Number(value) } : l));
    edited(true);
  }

  /** @param {string} stage @param {number} index @param {string} name */
  function setLutFile(stage, index, name) {
    if (!recipe) return;
    ensureMigration(recipe, stage);
    const key = lutsKey(stage);
    recipe[key] = (recipe[key] ?? []).map((/** @type {any} */ l, /** @type {number} */ i) => (i === index ? { ...l, name } : l));
    edited();
  }

  /** @param {boolean} [transient] @param {string} [key] */
  function edited(transient = false, key = undefined) {
    if (recipe) {
      if (!developEngine || developEngine === "none") developEngine = "spektra";
      recipe.engine = developEngine;
      emit("dev-panel-recipe-updated", { recipe: { ...recipe }, transient, key });
    }
  }

  /** @param {string} value */
  function engineChanged(value) {
    developEngine = value === "none" ? null : value;
    if (developEngine === null) {
      emit("dev-panel-engine-updated", { engine: null });
    } else {
      if (recipe) recipe.engine = developEngine;
      emit("dev-panel-engine-updated", { engine: developEngine });
    }
  }

  /** @param {string} key @param {number | string} v @param {boolean} transient @param {number} [index] */
  function setNum(key, v, transient, index) {
    if (!recipe) return;
    if (index != null) {
      if (!Array.isArray(recipe[key])) recipe[key] = [];
      recipe[key][index] = Number(v);
    } else {
      recipe[key] = Number(v);
    }
    edited(transient, key);
  }

  function resetRecipe() {
    emit("dev-panel-reset", {});
  }

  function hidePanel() {
    emit("dev-panel-close", {});
  }

  /** @param {string} key @param {number} [index] */
  function resetOne(key, index) {
    if (!defaults || !recipe || !(key in defaults)) return;
    if (index != null) {
      setNum(key, defaults[key]?.[index] ?? 0, false, index);
    } else {
      setNum(key, defaults[key], false);
    }
  }

  onMount(() => {
    if (!isTauri) {
      picked = "DSCF3201.RAF";
      photoPath = "/mnt/ffp-production/Capture/2026/2026-06-28/DSCF3201.RAF";
      renderMs = 532;
      rating = 3;
      films = [{ name: "gold200", label: "Kodak Gold 200" }];
      papers = [{ name: "portra_endura", label: "Kodak Portra Endura" }];
      recipe = {
        auto_exposure: false, exposure_ev: 0, print_exposure_ev: -0.05,
        whites: 0, highlights: 0, midtones: 0, shadows: 0, rolloff: 0,
        film: "gold200", paper: "portra_endura", y_shift: 0, m_shift: 0,
        grain: 1, halation: 1, halation_size: 1, diffusion: 1, sharpen: 1,
      };
      defaults = { ...recipe };
      developEngine = "spektra";
      return;
    }

    const unlisten = listen("main-dev-state", (e) => {
      const state = /** @type {any} */ (e.payload);
      photoPath = state.photoPath;
      picked = state.picked;
      recipe = state.recipe;
      developEngine = state.developEngine ?? null;
      renderMs = state.renderMs;
      status = state.status;
      installedEditors = state.installedEditors;
      exportEdge = state.exportEdge;
      exportBorder = state.exportBorder;
      exportFolder = state.exportFolder ?? "";
      films = state.films;
      papers = state.papers;
      luts = state.luts ?? [];
      engines = state.engines ?? [];
      caption = state.caption;
      tags = state.tags ?? [];
      rating = state.rating ?? 0;
      histogram = state.histogram ?? null;
      photoScale = state.photoScale ?? DEFAULT_PHOTO_SIZE;
      if (typeof state.checkLayer === "string") {
        checkLayer = state.checkLayer;
        showClipping = checkLayer !== "none";
      } else if (typeof state.showClipping === "boolean") {
        showClipping = state.showClipping;
        checkLayer = showClipping ? (lastActiveCheckLayer || "clipping") : "none";
      }
    }).then(() => {
      emit("dev-panel-ready", {});
    });

    invoke("default_recipe")
      .then((d) => (defaults = /** @type {any} */ (d)))
      .catch(() => {});

    let pointerInside = false;
    /** @param {boolean} inside */
    const setPresence = (inside) => {
      pointerInside = inside;
      invoke("set_focus_window_presence", { windowId: "dev-panel", inside }).catch(() => {});
    };
    const onFocus = () => {
      if (pointerInside) setPresence(true);
    };
    const onBlur = () => {
      pointerInside = false;
      setPresence(false);
    };
    const onPointerEnter = () => setPresence(true);
    const onPointerLeave = () => {
      pointerInside = false;
      setPresence(false);
    };
    const onPointerMove = () => {
      if (!pointerInside) setPresence(true);
    };
    window.addEventListener("pointerenter", onPointerEnter);
    window.addEventListener("pointerleave", onPointerLeave);
    window.addEventListener("pointermove", onPointerMove);
    document.addEventListener("mouseleave", onPointerLeave);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    const FORWARD_KEYS = new Set([
      "g", "s", "d", "z", "Escape", " ", "r", "f", "q",
      "0", "1", "2", "3", "4", "5",
      "o", "l", "b", "m",
      "=", "+", "-", "_",
      "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown",
    ]);
    /** @param {KeyboardEvent} e */
    const forwardKey = (e) => {
      /** @type {any} */
      const t = e.target;
      const tag = t?.tagName;
      if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA" || tag === "BUTTON") return;
      if (t?.getAttribute?.("role") === "button") return;
      if (!FORWARD_KEYS.has(e.key)) return;
      emit("dev-panel-key", {
        key: e.key,
        metaKey: e.metaKey,
        ctrlKey: e.ctrlKey,
        shiftKey: e.shiftKey,
      });
    };
    window.addEventListener("keydown", forwardKey);

    return () => {
      /** @type {(fn: any) => void} */
      const callUnlisten = (fn) => fn();
      unlisten.then(callUnlisten);
      window.removeEventListener("pointerenter", onPointerEnter);
      window.removeEventListener("pointerleave", onPointerLeave);
      window.removeEventListener("pointermove", onPointerMove);
      document.removeEventListener("mouseleave", onPointerLeave);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("keydown", forwardKey);
      setPresence(false);
    };
  });

  return {
    get photoPath() { return photoPath; },
    get picked() { return picked; },
    get recipe() { return recipe; },
    set recipe(v) { recipe = v; },
    get developEngine() { return developEngine; },
    get renderMs() { return renderMs; },
    get status() { return status; },
    get installedEditors() { return installedEditors; },
    get exportEdge() { return exportEdge; },
    set exportEdge(v) { exportEdge = v; },
    get exportBorder() { return exportBorder; },
    set exportBorder(v) { exportBorder = v; },
    get exportFolder() { return exportFolder; },
    get films() { return films; },
    get papers() { return papers; },
    get luts() { return luts; },
    get engines() { return engines; },
    get caption() { return caption; },
    set caption(v) { caption = v; },
    get tags() { return tags; },
    set tags(v) { tags = v; },
    get rating() { return rating; },
    get publishing() { return publishing; },
    set publishing(v) { publishing = v; },
    get publishStatus() { return publishStatus; },
    set publishStatus(v) { publishStatus = v; },
    get histogram() { return histogram; },
    get photoScale() { return photoScale; },
    set photoScale(v) { photoScale = v; },
    get showClipping() { return showClipping; },
    get checkLayer() { return checkLayer; },
    get showCaption() { return showCaption; },

    photoScaleChanged,
    toggleCheckLayer,
    toggleClipping,
    toggleCaptionOverlay,
    captionEdited,
    tagsEdited,
    addLutLayer,
    removeLutLayer,
    updateLutOpacity,
    setLutFile,
    edited,
    engineChanged,
    resetRecipe,
    hidePanel,
    resetOne,

    get panelProps() {
      return {
        photoPath,
        picked,
        get recipe() { return recipe; },
        set recipe(v) { recipe = v; },
        developEngine,
        renderMs,
        status,
        installedEditors,
        get exportEdge() { return exportEdge; },
        set exportEdge(v) { exportEdge = v; },
        get exportBorder() { return exportBorder; },
        set exportBorder(v) { exportBorder = v; },
        exportFolder,
        films,
        papers,
        luts,
        engines,
        get caption() { return caption; },
        set caption(v) { caption = v; },
        get tags() { return tags; },
        set tags(v) { tags = v; },
        rating,
        get publishing() { return publishing; },
        set publishing(v) { publishing = v; },
        get publishStatus() { return publishStatus; },
        set publishStatus(v) { publishStatus = v; },
        histogram,
        get photoScale() { return photoScale; },
        set photoScale(v) { photoScale = v; },
        onPhotoScaleChanged: photoScaleChanged,
        showClipping,
        checkLayer,
        onSelectCheckLayer: toggleCheckLayer,
        toggleClipping,
        showCaption,
        toggleCaptionOverlay,
        edited,
        resetOne,
        addLutLayer,
        removeLutLayer,
        updateLutOpacity,
        setLutFile,
        engineChanged,
        resetRecipe,
        hidePanel,
        onCaptionEdited: captionEdited,
        onTagsEdited: tagsEdited,
        onExportSettingsChanged: () => emit("dev-panel-export-settings-changed", { exportEdge, exportBorder }),
        onExport: () => emit("dev-panel-export", {}),
        onExportDaily: () => emit("dev-panel-export-daily", {}),
        onChooseExportFolder: () => emit("dev-panel-choose-export-folder", {}),
        onOpenInEditor: (/** @type {string} */ appPath) => emit("dev-panel-open-in-editor", { appPath }),
        onSetZoneMask: (/** @type {any} */ mask) => emit("dev-panel-set-zone-mask", { zoneMask: mask }),
        detached: true,
        onToggleDetached: () => emit("dev-panel-dock-requested", {}),
      };
    },
  };
}
