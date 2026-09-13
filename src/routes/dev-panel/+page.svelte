<script>
  // The develop panel — a 1:1 port of the Swift `DevPanel.swift` +
  // `Controls.swift` control language: DIN section titles with chevrons
  // (collapsible, remembered), SliderField rows (88px DIN label, thin ink
  // track, round thumb, mono value), StyledMenu pills, StyledToggle capsule,
  // and the I/D/E shape: INFO / DÉVELOPPEMENT / EXPORT.
  import { onMount } from "svelte";
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";
  import InfoBlock from "@modules/develop/tabs/InfoBlock.svelte";
  import ToolsStrip from "@modules/develop/tabs/ToolsStrip.svelte";
  import DevTab from "@modules/develop/tabs/DevTab.svelte";
  import CropTab from "@modules/develop/tabs/CropTab.svelte";
  import PresetTab from "@modules/develop/tabs/PresetTab.svelte";
  import ExportTab from "@modules/develop/tabs/ExportTab.svelte";

  /** @typedef {{ name: string, label: string }} FilmOrPaper */
  /** @typedef {{ name: string, opacity: number }} LutLayer */
  /** @typedef {{ id: string, label: string, control_groups?: any[] }} EngineInfo */
  /** @typedef {{ aperture?: number, shutter?: string, iso?: number, focal_mm?: number, captured_at?: string, make?: string, model?: string, width?: number, height?: number }} ExifInfo */
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
  /** @type {(string | LutLayer)[]} */ // .cube filenames (strings) from list_luts, via main-dev-state
  let luts = $state([]);
  let caption = $state("");
  let rating = $state(0);
  let publishing = $state(false);
  let publishStatus = $state("");
  /** @type {ExifInfo | null} */ // the frame's EXIF spec sheet, from `frame_info`
  let exif = $state(null);
  /** @type {Recipe | null} */ // engine defaults, for double-click-to-reset
  let defaults = $state(null);
  let showClipping = $state(false);

  function toggleClipping() {
    showClipping = !showClipping;
    emit("dev-panel-toggle-clipping", { showClipping });
  }

  function onCropClick() {
    activeTab = "crop";
  }

  function onPresetClick() {
    activeTab = "preset";
  }

  // Quick-export the current photo straight to the Desktop, bypassing
  // whatever custom export folder is configured — the main window's
  // exportCurrent() accepts a destDir override for exactly this.
  function onExportDesktopClick() {
    emit("dev-panel-export-desktop", {});
  }

  /** @type {string | null} */ // slider value being typed, by recipe key
  let editingKey = $state(null);

  // The engine registry comes from Rust — each engine (spektra, rapid, …)
  // declares its own id, label AND control_groups (its UI slice) via
  // `list_engines`. We render exactly what it reports; nothing is hardcoded
  // here, so a new Rust engine appears with zero changes to this file.
  /** EngineInfo[]: {id, label, control_groups} @type {EngineInfo[]} */
  let engines = $state([]);

  // developEngine holds the Rust engine id ("spektra" | "rapid" | null).
  let activeEngine = $derived(engines.find((e) => e.id === developEngine));

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
  function openLutsDir() {
    invoke("luts_dir").then((dir) => invoke("open_path", { path: dir })).catch(() => {});
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

  let activeTab = $state("dev");

  onMount(() => {
    // Plain-browser preview (vite dev, no Tauri): show a demo state so the
    // panel's control language is inspectable — the real window overwrites
    // all of this through `main-dev-state`.
    if (!isTauri) {
      picked = "DSCF3201.RAF";
      photoPath = "/mnt/ffp-production/Capture/2026/2026-06-28/DSCF3201.RAF";
      renderMs = 532;
      rating = 3;
      exif = {
        aperture: 5.6, shutter: "1/60", iso: 1000, focal_mm: 23,
        captured_at: "2026-06-28 · 14:25", make: "FUJIFILM", model: "X-T5",
      };
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
      rating = state.rating ?? 0;
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
    const onFocus = () => {
      if (pointerInside) setPresence(true);
    };
    const onBlur = () => setPresence(false);
    const onPointerEnter = () => setPresence(true);
    const onPointerLeave = () => setPresence(false);
    window.addEventListener("pointerenter", onPointerEnter);
    window.addEventListener("pointerleave", onPointerLeave);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    // Forward global navigation shortcuts to the main window — this panel is a
    // separate OS window that holds keyboard focus while it's up, so g/s/z/
    // arrows/… would otherwise be swallowed here. Skip when a control has
    // focus (it owns its own keys) and only forward the navigation whitelist.
    const FORWARD_KEYS = new Set([
      "g", "s", "d", "z", "Escape", " ", "r", "f", "q",
      "0", "1", "2", "3", "4", "5",
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

  function captionEdited() {
    emit("dev-panel-caption-updated", { caption });
  }

  function exportSettingsChanged() {
    emit("dev-panel-export-settings-changed", { exportEdge, exportBorder });
  }

  function triggerExport() {
    emit("dev-panel-export", {});
  }

  function resetRecipe() {
    emit("dev-panel-reset", {});
  }

  /** @param {string} appPath */
  function openInEditor(appPath) {
    if (appPath) emit("dev-panel-open-in-editor", { appPath });
  }

  function hidePanel() {
    emit("dev-panel-close", {});
  }

  function copyPath() {
    if (photoPath) navigator.clipboard?.writeText(photoPath);
  }

  // PUBLIER — develop, upload, PUT a one-image note to the Garden. The
  // credentials come from the vault's garden plugin config, Rust-side.
  async function publishPhoto() {
    if (!photoPath || publishing || !isTauri) return;
    publishing = true;
    publishStatus = "";
    try {
      const live = await invoke("publish_photo", { path: photoPath });
      publishStatus = live;
    } catch (e) {
      publishStatus = String(e);
    } finally {
      publishing = false;
    }
  }

  function revealInFinder() {
    if (photoPath && isTauri) {
      invoke("open_path", { path: photoPath.split("/").slice(0, -1).join("/") }).catch(() => {});
    }
  }

  // EXIF arrives per photo — cheap metadata-only read, no pixel decode.
  $effect(() => {
    const p = photoPath;
    if (!isTauri) return;
    exif = null;
    if (p) {
      invoke("frame_info", { path: p })
        .then((i) => {
          if (p === photoPath) exif = i;
        })
        .catch(() => {});
    }
  });

  // "ƒ5.6  1/60  ISO 1000  23mm" — the Swift `exposure.spec` line.
  const exifLine = $derived.by(() => {
    if (!exif) return "";
    const parts = [];
    if (exif.aperture) parts.push(`ƒ${Number(exif.aperture.toFixed(1))}`);
    if (exif.shutter) parts.push(exif.shutter);
    if (exif.iso) parts.push(`ISO ${exif.iso}`);
    if (exif.focal_mm) parts.push(`${Math.round(exif.focal_mm)}mm`);
    return parts.join("   ");
  });

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

  /** @param {string} key @param {number | string} raw @param {number} min @param {number} max */
  function commitEdit(key, raw, min, max) {
    const v = Number(String(raw).replace(",", "."));
    if (Number.isFinite(v)) setNum(key, Math.min(Math.max(v, min), max), false);
    editingKey = null;
  }

  /** @param {HTMLInputElement} node */
  const autofocus = (node) => {
    node.focus();
    node.select();
  };

  /** @param {number | string} v */
  const fmt = (v) => {
    const s = Number(v).toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
    return s === "" || s === "-0" ? "0" : s;
  };
  /** @param {number | string} v @param {number} min @param {number} max */
  const pct = (v, min, max) => `${((Number(v) - min) / (max - min)) * 100}%`;
  /** @param {string | null} p */
  const parentDir = (p) => (p ? p.split("/").slice(0, -1).join("/").replace(/^\/Users\/[^/]+/, "~") : "");
</script>

<div class="panel">
  <!-- STICKY TOP ZONE -->
  <div class="sticky-top">
    <header data-tauri-drag-region>
      <button class="close" onclick={hidePanel} title="Fermer le panneau (⇧D)">
        <Icon name="x" size="11px" />
      </button>
      <span class="din title">{picked ?? "—"}</span>
      <button
        class="header-util-btn"
        class:active={showClipping}
        onclick={toggleClipping}
        title="Avertissement d'écrêtage (Blancs & Noirs)"
      >
        <Icon name="circle-half" size="12px" />
        {#if showClipping}
          <span class="clip-indicator"></span>
        {/if}
      </button>
    </header>

    <!-- TAB BAR -->
    <div class="tab-bar">
      <button class="tab-btn" class:active={activeTab === 'dev'} onclick={() => activeTab = 'dev'}>
        <Icon name="sliders-horizontal" size="11px" />
        <span>Dev</span>
      </button>
      <button class="tab-btn" class:active={activeTab === 'crop'} onclick={() => activeTab = 'crop'}>
        <Icon name="crop" size="11px" />
        <span>Crop</span>
      </button>
      <button class="tab-btn" class:active={activeTab === 'preset'} onclick={() => activeTab = 'preset'}>
        <Icon name="stack-simple" size="11px" />
        <span>Presets</span>
      </button>
      <button class="tab-btn" class:active={activeTab === 'info'} onclick={() => activeTab = 'info'}>
        <Icon name="image" size="11px" />
        <span>Info</span>
      </button>
      <button class="tab-btn" class:active={activeTab === 'export'} onclick={() => activeTab = 'export'}>
        <Icon name="download-simple" size="11px" />
        <span>Export</span>
      </button>
    </div>
    <div class="hairline"></div>
  </div>

  <!-- SCROLLABLE TAB CONTENT -->
  <div class="pane-scroll">
    {#if activeTab === 'dev'}
      <DevTab
        bind:recipe
        {engines}
        {developEngine}
        {activeEngine}
        {films}
        {papers}
        {luts}
        {edited}
        {resetOne}
        {addLutLayer}
        {removeLutLayer}
        {updateLutOpacity}
        {setLutFile}
        {engineChanged}
        {resetRecipe}
      />
    {:else if activeTab === 'crop'}
      <CropTab bind:recipe {edited} />
    {:else if activeTab === 'preset'}
      <PresetTab bind:recipe />
    {:else if activeTab === 'info'}
      <InfoBlock
        {picked}
        {exifLine}
        {exif}
        {renderMs}
        {status}
        {rating}
        bind:caption
        {photoPath}
      />
    {:else if activeTab === 'export'}
      <ExportTab
        {installedEditors}
        {exportFolder}
        bind:exportEdge
        bind:exportBorder
        {photoPath}
        bind:publishing
        bind:publishStatus
      />
    {/if}
  </div>
</div>

<style>
  :global(body) {
    background-color: var(--color-surface-high);
    color: var(--color-foreground);
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  .panel {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--color-surface-high);
  }

  .sticky-top {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-surface-high);
    z-index: 10;
  }

  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px 10px;
  }
  .close {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    transition: color var(--duration-fast), background var(--duration-fast);
  }
  .close:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
  }
  header .title {
    color: var(--color-foreground);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    flex: 1;
  }
  .header-util-btn {
    all: unset;
    cursor: pointer;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm, 4px);
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    transition: color var(--duration-fast), background var(--duration-fast);
  }
  .header-util-btn:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .header-util-btn.active {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }
  .clip-indicator {
    position: absolute;
    bottom: 2px;
    right: 2px;
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--color-accent);
  }

  .hairline {
    height: 1px;
    background: var(--color-border);
    margin: 0 12px;
    flex-shrink: 0;
  }

  .tab-bar {
    display: flex;
    gap: 3px;
    margin: 0 12px 8px;
    padding: 2px;
    background: color-mix(in srgb, var(--color-foreground) 3.5%, transparent);
    border-radius: var(--radius-sm, 4px);
  }
  .tab-btn {
    all: unset;
    cursor: pointer;
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 4px 0;
    border-radius: 3px;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    transition: color var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft), box-shadow var(--duration-fast) var(--ease-soft);
  }
  .tab-btn:hover {
    color: var(--color-foreground);
  }
  .tab-btn.active {
    color: var(--color-foreground);
    background: var(--color-surface-high);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    /* padding is handled within the individual tab components */
  }

</style>
