<script>
  // The develop panel's DETACHED host — a real, separate Tauri window with no
  // shared JS memory with the main window, so every piece of state below is
  // a local MIRROR kept in sync over `main-dev-state`/`dev-panel-*` events.
  // The actual interface lives in DevelopPanel.svelte (shared with the
  // eventual docked host); this file's only job is the IPC bridge plus the
  // things that only make sense for a separate OS window — forwarding
  // keyboard shortcuts the main window would otherwise never see, and
  // reporting focus/pointer presence for focus-mode dimming.
  import { onMount } from "svelte";
  import { DEFAULT_PHOTO_SIZE } from "$lib/session.js";
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import DevelopPanel from "@modules/develop/DevelopPanel.svelte";

  /** @typedef {{ name: string, label: string }} FilmOrPaper */
  /** @typedef {{ id: string, label: string, control_groups?: any[] }} EngineInfo */
  /** @typedef {Record<string, any>} Recipe */

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
  let exportFolder = $state(""); // "" = the Desktop (Swift default)
  /** @type {FilmOrPaper[]} */
  let films = $state([]);
  /** @type {FilmOrPaper[]} */
  let papers = $state([]);
  /** @type {(string | {name: string, opacity: number})[]} */ // .cube filenames (strings) from list_luts, via main-dev-state
  let luts = $state([]);
  /** @type {EngineInfo[]} */
  let engines = $state([]);
  let caption = $state("");
  /** @type {string[]} */
  let tags = $state([]);
  let rating = $state(0);
  let publishing = $state(false);
  let publishStatus = $state("");
  /** @type {Recipe | null} */ // engine defaults, for double-click-to-reset
  let defaults = $state(null);
  let showClipping = $state(false);
  let showCaption = $state(false);
  /** @type {{r: number[], g: number[], b: number[], luma: number[]} | null} */
  let histogram = $state(null);
  let photoScale = $state(DEFAULT_PHOTO_SIZE);

  function photoScaleChanged() {
    emit("dev-panel-photo-scale-changed", { photoScale });
  }

  function toggleClipping() {
    showClipping = !showClipping;
    emit("dev-panel-toggle-clipping", { showClipping });
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

  // ---- LUT stacks (Rapid-only) — model matches Rust `LutLayer { name, opacity }`
  /** @param {string} stage */
  const lutsKey = (stage) => (stage === "pre" ? "rapid_pre_luts" : "rapid_post_luts");
  /** @param {string} stage */
  const oldLutsKey = (stage) => (stage === "pre" ? "pre_luts" : "post_luts");
  /** @param {Recipe} recipe @param {string} stage */
  function ensureMigration(recipe, stage) {
    if (!recipe || developEngine !== "rapid") return;
    const key = lutsKey(stage);
    const oldKey = oldLutsKey(stage);
    if (recipe[oldKey]?.length > 0) {
      recipe[key] = [...(recipe[key] ?? []), ...recipe[oldKey]];
      recipe[oldKey] = [];
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

  onMount(() => {
    // Plain-browser preview (vite dev, no Tauri): show a demo state so the
    // panel's control language is inspectable — the real window overwrites
    // all of this through `main-dev-state`.
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
    // Await listener registration BEFORE announcing readiness, otherwise the
    // main window's `main-dev-state` reply races past our listener and the
    // panel renders empty (black) — recipe never arrives.
    const unlisten = listen("main-dev-state", (e) => {
      const state = e.payload;
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
    }).then(() => {
      emit("dev-panel-ready", {});
    });

    invoke("default_recipe")
      .then((d) => (defaults = d))
      .catch(() => {});

    let pointerInside = false;
    /** @param {boolean} inside */
    const setPresence = (inside) => {
      pointerInside = inside;
      invoke("set_focus_window_presence", { windowId: "dev-panel", inside }).catch(() => {});
    };
    const onFocus = () => setPresence(true);
    const onBlur = () => {
      if (!pointerInside) setPresence(false);
    };
    const onPointerEnter = () => setPresence(true);
    const onPointerLeave = () => {
      pointerInside = false;
      if (!document.hasFocus()) setPresence(false);
    };
    const onPointerMove = () => {
      if (!pointerInside) setPresence(true);
    };
    window.addEventListener("pointerenter", onPointerEnter);
    window.addEventListener("pointerleave", onPointerLeave);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    // Forward global navigation shortcuts to the main window — this panel is a
    // separate OS window that holds keyboard focus while it's up, so g/s/z/
    // arrows/… would otherwise be swallowed here. Skip when a control has
    // focus (it owns its own keys) and only forward the navigation whitelist.
    const FORWARD_KEYS = new Set([
      "g", "s", "d", "z", "Escape", " ", "r", "f", "q",
      "0", "1", "2", "3", "4", "5",
      "o", "l", "b", "m",
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
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("keydown", forwardKey);
      setPresence(false);
    };
  });

  /** @param {boolean} [transient] @param {string} [key] */
  function edited(transient = false, key = undefined) {
    if (recipe) {
      // Touching a control engages a develop engine — DEFAULT to spektra, but
      // never clobber an explicit "rapid" choice (that killed fast-render mode
      // on every edit). developEngine holds the Rust engine id directly.
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
      // Carry the engine id to the main window so it picks the right render
      // path (rapid → develop_preview_rgba, else → develop_preview).
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

  // Double-click a slider label → back to the engine default for that one key
  // (or one band of an array-valued key, e.g. hsl_hue[i], when index is set).
  /** @param {string} key @param {number} [index] */
  function resetOne(key, index) {
    if (!defaults || !recipe || !(key in defaults)) return;
    if (index != null) {
      setNum(key, defaults[key]?.[index] ?? 0, false, index);
    } else {
      setNum(key, defaults[key], false);
    }
  }
</script>

<DevelopPanel
  {photoPath}
  {picked}
  bind:recipe
  {developEngine}
  {renderMs}
  {status}
  {installedEditors}
  bind:exportEdge
  bind:exportBorder
  {exportFolder}
  {films}
  {papers}
  {luts}
  {engines}
  bind:caption
  bind:tags
  {rating}
  bind:publishing
  bind:publishStatus
  {histogram}
  bind:photoScale
  onPhotoScaleChanged={photoScaleChanged}
  {showClipping}
  {toggleClipping}
  {showCaption}
  {toggleCaptionOverlay}
  {edited}
  {resetOne}
  {addLutLayer}
  {removeLutLayer}
  {updateLutOpacity}
  {setLutFile}
  {engineChanged}
  {resetRecipe}
  {hidePanel}
  onCaptionEdited={captionEdited}
  onTagsEdited={tagsEdited}
  onExportSettingsChanged={() => emit("dev-panel-export-settings-changed", { exportEdge, exportBorder })}
  onExport={() => emit("dev-panel-export", {})}
  onExportDaily={() => emit("dev-panel-export-daily", {})}
  onChooseExportFolder={() => emit("dev-panel-choose-export-folder", {})}
  onOpenInEditor={(/** @type {string} */ appPath) => emit("dev-panel-open-in-editor", { appPath })}
  detached={true}
  onToggleDetached={() => emit("dev-panel-dock-requested", {})}
/>

<style>
  :global(body) {
    background-color: var(--color-surface-high);
    color: var(--color-foreground);
    margin: 0;
    padding: 0;
    overflow: hidden;
    height: 100vh;
  }
</style>
