<script>
  import { tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen as tauriListen, emit } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { getCurrentWindow, currentMonitor, availableMonitors } from "@tauri-apps/api/window";
  import { LogicalPosition } from "@tauri-apps/api/dpi";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";
  import Sidebar from "@modules/sidebar/Sidebar.svelte";
  import CullView from "@modules/culling/CullView.svelte";
  import DevelopView from "@modules/develop/DevelopView.svelte";
  import StoryView from "@modules/story/StoryView.svelte";
  import ShortcutsModal from "@modules/modals/ShortcutsModal.svelte";
  import SettingsModal from "@modules/modals/SettingsModal.svelte";
  import RenderQueueModal from "@modules/modals/RenderQueueModal.svelte";
  import ContextMenu from "@modules/menus/ContextMenu.svelte";
  import { AppController } from "$lib/controllers/AppController.js";
  import { extractGardenUrl } from "$lib/story.js";

  // ---- shared shapes (plain-JS JSDoc typing — no runtime effect) ----------
  /** A catalogue frame row, as returned by `index_frames` / `list_dir`. */
  /** @typedef {Object} Frame
   * @property {string} path
   * @property {string} name
   * @property {number} rating
   * @property {number} [previewVersion]
   * @property {string | number} [capture_at]
   * @property {number} [aperture]
   * @property {string} [shutter]
   * @property {number} [iso]
   * @property {number} [focal_mm]
   * @property {string} [captured_at]
   * @property {string} [make]
   * @property {string} [model]
   */
  /** A directory row from `index_dirs`. */
  /** @typedef {Object} Dir
   * @property {string} dir
   */
  /** A removable card, as returned by `find_cards`. */
  /** @typedef {Object} Card
   * @property {string} [volume]
   * @property {string} name
   * @property {string} dcim
   * @property {number} raw_count
   */
  /** A develop recipe — the engine_settings blob persisted to sidecars. */
  /** @typedef {Object} Recipe
   * @property {string} engine
   * @property {boolean} auto_exposure
   * @property {number} exposure_ev
   * @property {number} print_exposure_ev
   * @property {number} whites
   * @property {number} highlights
   * @property {number} midtones
   * @property {number} shadows
   * @property {number} rolloff
   * @property {string} film
   * @property {string} paper
   * @property {number} y_shift
   * @property {number} m_shift
   * @property {number} grain
   * @property {number} halation
   * @property {number} halation_size
   * @property {number} diffusion
   * @property {number} sharpen
   */
  /** Long-running operation progress (import/export/publish/move). */
  /** @typedef {Object} Progress
   * @property {string} verb
   * @property {number} done
   * @property {number} total
   * @property {string} [current]
   * @property {string} [path]
   */
  /** The photo grid context-menu position/target. */
  /** @typedef {Object} PhotoMenu
   * @property {Frame} frame
   * @property {number} x
   * @property {number} y
   * @property {boolean} submenuLeft
   */
  /** A film/paper profile from `list_profiles`. */
  /** @typedef {Object} Profile
   * @property {string} stage
   * @property {string} name
   * @property {string} label
   */
  /** An engine registry entry from `list_engines`. */
  /** @typedef {Object} EngineInfo
   * @property {string} id
   * @property {string} label
   * @property {any} control_groups
   */
  /** A render-queue job. */
  /** @typedef {Object} ExportJob
   * @property {string} id
   * @property {string} label
   * @property {string} current
   * @property {number} done
   * @property {number} total
   * @property {string} phase
   * @property {string} status
   * @property {string} timestamp
   */
  /** Sidebar Garden account state. */
  /** @typedef {Object} GardenAccount
   * @property {boolean} signed_in
   * @property {string} [username]
   * @property {string} [tier]
   * @property {number} notes_count
   * @property {number} total_views
   */
  /** A story-note summary, from `list_story_notes`. */
  /** @typedef {Object} StoryNote
   * @property {string} notePath
   * @property {boolean} [pinned]
   * @property {string} [pinnedAt]
   * @property {number} mtime
   */
  /** A developed RGBA preview buffer returned by `develop_preview_rgba`. */
  /** @typedef {Object} RgbaPreview
   * @property {number} width
   * @property {number} height
   * @property {ArrayBuffer | number[]} rgba
   */
  /**
   * @typedef {Window & typeof globalThis & {
   *   __log?: (...args: any[]) => void;
   *   currentMonitor?: () => Promise<any> | any;
   * }} RevealWindow
   */

  const PREVIEW_PX = 2048;
  // Live-drag proxy resolution. Kept low so the CPU rapid/AgX engine can keep
  // up frame-to-frame while a slider moves; it snaps back to PREVIEW_PX the
  // instant the drag settles. 1100 was too heavy for the per-pixel CPU path.
  const DRAG_PX = 768;

  /**
   * Register a Tauri event listener. Vite-HMR tolerant: a discarded page's
   * failed registration is swallowed (no noisy console when hot-reloading).
   * @param {string} event
   * @param {(payload: any) => void} handler
   * @returns {Promise<() => void>}
   */
  function listen(event, handler) {
    return tauriListen(event, handler).catch((error) => {
      // Vite can replace the webview before Rust returns the listener callback
      // ID. The discarded page cannot use that listener, so this is harmless.
      if (!import.meta.hot) console.error(`Could not register ${event}:`, error);
      return () => {};
    });
  }

  /** @type {"cull" | "dev" | "story"} */
  let currentMode = $state("cull");
  // Space is a "loupe" — the single-photo canvas without the dev panel. While a
  // space-look is active we suppress the panel window regardless of the stored
  // layouts.dev.devPanel preference, so quick looks never pop (or steal focus
  // to) the panel. Any deliberate develop entry (d, click a photo, ⇧D) clears it.
  let spaceLook = $state(false);
  // Single-photo zoom cycle (Z / ⇧Z): the framed look with white space around
  // it → the photo filling the viewport edge-to-edge → 1:1 actual pixels → back.
  const ZOOM_CYCLE = ["frame", "fill", "actual"];
  let zoomMode = $state("frame"); // one of ZOOM_CYCLE
  let developViewport = $state({ width: 0, height: 0 });
  let fullscreen = $state(false);
  /** @type {string | null} */ let fullscreenUrl = $state(null);
  let fullscreenRequest = 0;
  // Pointer/focus presence for the window — drives focus mode AND the
  // fullscreen traffic-light hint, which fades out when the mouse leaves.
  let pointerInside = $state(false);
  const developPhotoPercent = $derived.by(() => {
    const minDimension = Math.min(developViewport.width, developViewport.height);
    const progress = Math.min(Math.max((minDimension - 500) / 600, 0), 1);
    const factor = 1.05 + (1.618 - 1.05) * progress;
    return 100 / factor;
  });
  /** @type {Record<"cull" | "dev" | "story", Record<string, boolean>>} */
  let layouts = $state({
    cull: { sidebar: true, focus: false, devPanel: false },
    dev: { sidebar: false, focus: false, devPanel: true },
    story: { sidebar: true, focus: false, devPanel: false }
  });
  /** @type {string | null} */ let folder = $state(null);
  // RAW, not deep-reactive: the contact sheet holds up to ~22k frames, and a
  // plain $state would wrap every element in a Proxy — then any full-array pass
  // (view's sort, selectedFrames' filter) fires millions of proxy traps and
  // freezes the folder open. Raw means only reassigning `frames` is reactive,
  // so in-place edits (rating, previewVersion) reassign with `frames = [...frames]`.
  /** @type {Frame[]} */ let frames = $state.raw([]);
  let loading = $state(false); // a folder open is in flight — suppresses the empty-state splash so switching folders doesn't flash "REVEAL"
  let sel = $state(0);
  /** @type {Set<string>} */ let selectedPaths = $state(new Set());
  let selectionAnchor = $state(0);
  /** @type {Recipe | null} */ let copiedRecipe = $state(null);
  /** @type {string[]} */ let roots = $state([]); // every catalogue root — the library is a SET of them
  /** @type {string | null} */ let root = $derived(roots[0] ?? null); // primary root, for legacy single-root paths
  /** @type {string | null} */ let importDir = $state(null); // chosen import folder (null → fall back to root)
  /** @type {Dir[]} */ let dirs = $state([]);
  /** @type {string | null} */ let curDir = $state(null);
  let minRating = $state(0);
  let scanning = $state(false);
  /** @type {Card[]} */ let cards = $state([]);
  /** @type {Progress | null} */ let progress = $state(null); // {verb, done, total, current}
  // The Swift export prefs, faithfully: long edge Plein/4096/2048/1600/1024
  // (default 2048), border toggle, and a remembered export folder ("" = the
  // Desktop, resolved Rust-side) — no dialog on every export.
  let exportEdge = $state(2048);
  let exportBorder = $state(false);
  let exportFolder = $state("");
  let appMessage = $state("");
  let autoImport = $state(false);
  // AI cull (walk-away card→cull→export): mirrors autoImport's hydrate-at-
  // boot pattern, but sourced from the generic preferences bag rather than
  // a dedicated ShellPrefs field (see SettingsModal's "AI CULL" section).
  let aiCullEnabled = $state(false);
  let aiCullTarget = $state(24);
  // Per-run bookkeeping: destDir -> file paths copied THIS import, built
  // from import-progress's per-file payload (already emitted, previously
  // unused beyond destDir) — the exact candidate pool for ai_cull, robust to
  // a folder that already had photos in it before this run.
  /** @type {Map<string, string[]>} */ let importedByFolder = new Map();
  /** @type {string | null} */ let lastImportedFolder = $state(null);
  // The card currently importing (has {volume, name, dcim}) — kept so the
  // rail can stop the run and, once done, offer to eject that same card.
  /** @type {Card | null} */ let importingCard = $state(null);
  // A finished card import awaiting ejection — the ingest loop's last step.
  /** @type {Card | null} */ let ejectableCard = $state(null);
  let ejecting = $state(false);
  let layout = $state("uniform"); // "uniform" | "masonry"
  let baseOpen = $state(true);
  let tonalityOpen = $state(true);
  let filmOpen = $state(true);
  let textureOpen = $state(true);
  let shortcutsOpen = $state(false);
  let settingsOpen = $state(false);
  // Sidebar Garden account row — null until boot resolves the stored key.
  /** @type {GardenAccount | null} */ let gardenAccount = $state(null);
  let preferences = $state({
    date_folders: "%Y/%Y-%m-%d",
    vault: "",
    logs_folder: "Logs",
    export_folder: "",
    lut_folder: "",
    ai_cull_enabled: false,
    ai_cull_target: 24,
    ai_api_key: "",
  });

  // Grid geometry + rail filters — the Swift model's columnsPref/cellAspect/
  // fillCells/marginScale/minStars/filterStory/sortMode, persisted together.
  let cols = $state(4);
  let marginScale = $state(1);
  let cellAspect = $state(1.5);
  let fillCells = $state(true);
  let filterStory = $state(false);
  let sortDesc = $state(false);
  let starMenuOpen = $state(false);
  let layoutMenuOpen = $state(false);
  let sortMenuOpen = $state(false);
  let storyDirs = $state(new Set());
  // Story-notes sidebar state. pinnedStories = the pinned-list order
  // (frontmatter pinned-at descending — newest/newest-pinned at top);
  // recentStories = newest 12 by mtime.
  /** @type {StoryNote[]} */
  let pinnedStories = $state([]);
  /** @type {StoryNote[]} */
  let recentStories = $state([]);
  /** @type {{ dirs: number; frames: number } | null} */ let indexProgress = $state(null); // {dirs, frames} while a scan walks

  /** @type {string | null} */ let photoPath = $state(null);
  /** @type {string | null} */ let picked = $state(null);
  const currentRating = $derived(frames.find((f) => f.path === photoPath)?.rating ?? 0);
  /** @type {string | null} */ let imgUrl = $state(null);
  /** @type {HTMLCanvasElement | null} */ let canvasEl = $state(null);
  let useCanvas = $state(false);
  /** @type {number | null} */ let renderAspect = $state(null); // width/height of the last canvas (rapid) render — reactive aspect for the loupe
  let imgFailed = $state(false); // loupe image couldn't decode/load (NAS drop, junk file…)
  let status = $state("");
  /** @type {number | null} */ let renderMs = $state(null);
  /** @type {Profile[]} */ let films = $state([]);
  /** @type {Profile[]} */ let papers = $state([]);
  /** @type {any[]} */ let luts = $state([]);
  /** @type {EngineInfo[]} */ let engines = $state([]); // EngineInfo[] from Rust: {id, label, control_groups} — the UI slice per engine
  /** @type {Recipe | null} */ let recipe = $state(null);
  /** @type {string | null} */ let developEngine = $state(null);
  let lastEditedKey = $state("exposure_ev");

  /** @type {Record<string, { step: number, shiftStep: number }>} */
  const SETTING_STEPS = {
    exposure_ev: { step: 0.05, shiftStep: 0.25 },
    print_exposure_ev: { step: 0.05, shiftStep: 0.25 },
    temperature: { step: 50, shiftStep: 250 },
    tint: { step: 1, shiftStep: 5 },
    y_shift: { step: 1, shiftStep: 5 },
    m_shift: { step: 1, shiftStep: 5 },
    c_shift: { step: 1, shiftStep: 5 },
    contrast: { step: 1, shiftStep: 5 },
    highlights: { step: 1, shiftStep: 5 },
    shadows: { step: 1, shiftStep: 5 },
    whites: { step: 1, shiftStep: 5 },
    midtones: { step: 1, shiftStep: 5 },
    saturation: { step: 1, shiftStep: 5 },
    vibrance: { step: 1, shiftStep: 5 },
    grain: { step: 0.1, shiftStep: 0.5 },
    halation: { step: 0.1, shiftStep: 0.5 },
    diffusion: { step: 0.1, shiftStep: 0.5 },
    sharpen: { step: 0.1, shiftStep: 0.5 },
  };
  let showClipping = $state(false);
  let caption = $state("");
  /** @type {Set<string>} */ let storySet = $state(new Set());
  /** @type {string | null} */ let liveUrl = $state(null);
  let storyContent = $state("");
  let gardenUrl = $derived(extractGardenUrl(storyContent) || liveUrl);

  /** @type {Record<string, number>} */ let scrollOffsets = $state({});
  /** @type {[string, string][]} */ let installedEditors = $state([]);
  let catalogContent = $state("");
  let catalogOpen = $state(false);
  /** @type {ExportJob[]} */ let exportQueue = $state([]);
  /** @type {string | null} */ let activeExportJobId = $state(null);
  let queueOpen = $state(false);
  let currentScrollTop = $state(0);
  /** @type {PhotoMenu | null} */ let photoMenu = $state(null);

  $effect(() => {
    const d = gridDir();
    if (d && currentMode === "cull") {
      scrollOffsets[d] = currentScrollTop;
    }
  });

  // ---- boot -------------------------------------------------------------
  $effect(() => {
    if (typeof localStorage !== "undefined") {
      const savedLayouts = localStorage.getItem("reveal.modeLayouts");
      if (savedLayouts) {
        try {
          const parsed = JSON.parse(savedLayouts);
          if (parsed && typeof parsed === "object") {
            // Backfill palette flags on the PLAIN parsed object, before the
            // assign. Reading them off `layouts` here would make this boot
            // effect depend on state it also writes (the assign replaces
            // layouts.dev each run) — a self-invalidating effect that re-ran
            // forever, hammering openDir/refreshDirs in a loop.
            if (parsed.dev && typeof parsed.dev === "object") {
              // (Legacy presetPanel/lutPanel ignored)
            }
            Object.assign(layouts, parsed);
          }
        } catch (e) {}
      }

      // Reveal always reopens into the contact sheet. The last workflow mode
      // remains available per folder, but never hijacks startup.
      currentMode = "cull";

      const saved = localStorage.getItem("reveal.layout");
      if (saved === "uniform" || saved === "masonry") {
        layout = saved;
      }
      try {
        const x = JSON.parse(localStorage.getItem("reveal.export") ?? "null");
        if (x && typeof x === "object") {
          if ([0, 4096, 2048, 1600, 1024].includes(x.edge)) exportEdge = x.edge;
          if (typeof x.border === "boolean") exportBorder = x.border;
          if (typeof x.folder === "string") exportFolder = x.folder;
        }
      } catch (e) {}
      try {
        const g = JSON.parse(localStorage.getItem("reveal.grid") ?? "null");
        if (g && typeof g === "object") {
          // Any 1–12 count is valid — the keyboard zoom (+/−) steps through
          // every integer, not just the popover's presets.
          const savedCols = Math.trunc(Number(g.cols));
          if (Number.isFinite(savedCols) && savedCols >= 1 && savedCols <= 12) {
            cols = savedCols;
          }
          const savedMargin = Number(g.marginScale);
          if (Number.isFinite(savedMargin)) {
            marginScale = Math.min(6, Math.max(0.25, savedMargin));
          }
          const savedAspect = Number(g.cellAspect);
          if (Number.isFinite(savedAspect)) {
            cellAspect = Math.min(3, Math.max(0.5, savedAspect));
          }
          if (typeof g.fillCells === "boolean") fillCells = g.fillCells;
          if (typeof g.sortDesc === "boolean") sortDesc = g.sortDesc;
        }
      } catch (e) {}
    }
    if (!isTauri) return;
    (async () => {
      /** @type {RevealWindow} */ (window).__log?.(`viewport ${innerWidth}x${innerHeight} dpr=${devicePixelRatio} body=${getComputedStyle(document.body).width}`);
      recipe = await invoke("default_recipe");
      /** @type {Profile[]} */
      const profiles = await invoke("list_profiles");
      await loadExternalEditors();
      films = profiles.filter((p) => p.stage === "filming");
      papers = profiles.filter((p) => p.stage === "printing");
      luts = await invoke("list_luts").catch(() => []);
      // Each engine declares its own id, label AND control_groups (its UI) —
      // the frontend renders whatever the Rust registry reports, so adding an
      // engine is a Rust module with zero frontend changes.
      engines = await invoke("list_engines").catch(() => []);
      await refreshDirs();
      const lastDir = localStorage.getItem("reveal.lastDirectory");
      if (lastDir) {
        await openDir(lastDir, false);
      }
      currentMode = "cull";
      await switchMode("cull");
      // A seeded root with an empty index (fresh install, or the library was
      // pointed elsewhere) indexes itself — no button hunt on first launch.
      if (root && !dirs.length) rescan();
      const shellPrefs = await invoke("load_shell_prefs");
      layouts[currentMode].focus = !!shellPrefs.focus_mode;
      autoImport = !!shellPrefs.auto_import;
      importDir = shellPrefs.import_dir ?? null;
      const savedPreferences = await invoke("load_preferences").catch(() => ({}));
      aiCullEnabled = !!savedPreferences.ai_cull_enabled;
      aiCullTarget = Number(savedPreferences.ai_cull_target) || 24;
      // The Garden account row: cached username shows instantly, the silent
      // /me re-verify refreshes stats (Swift `refreshIfNeeded`).
      if (shellPrefs.garden_username) {
        gardenAccount = { signed_in: true, username: shellPrefs.garden_username, tier: shellPrefs.garden_tier, notes_count: 0, total_views: 0 };
      }
      invoke("garden_refresh").then((info) => (gardenAccount = info)).catch(() => {});
      listen("garden-account-changed", (e) => (gardenAccount = e.payload));
      listen("open-file-requested", (e) => openPhoto(e.payload));
      listen("dock-reopen-requested", () => switchMode("cull"));
      listen("focus-mode-changed", (e) => {
        layouts[currentMode].focus = !!e.payload.enabled;
        saveLayouts();
      });
      listen("shell-prefs-changed", (e) => {
        autoImport = !!e.payload.auto_import;
        importDir = e.payload.import_dir ?? null;
        layouts[currentMode].focus = !!e.payload.focus_mode;
        saveLayouts();
      });
      // The scan commits per directory now — refresh the sidebar tree on a
      // slow cadence while it walks, so folders appear as they're indexed.
      let lastTreeRefresh = 0;
      listen("index-progress", (e) => {
        if (e.payload.done) {
          indexProgress = null;
          refreshDirs();
          return;
        }
        indexProgress = e.payload;
        const now = Date.now();
        if (now - lastTreeRefresh > 2500) {
          lastTreeRefresh = now;
          refreshDirs(true);
        }
      });
      listen("cards-changed", (e) => (cards = e.payload));
      listen("card-mounted", (e) => {
        const card = e.payload;
        appMessage = `carte détectée · ${card.name} (${card.raw_count})`;
        setTimeout(() => (appMessage = ""), 4000);
        if (autoImport && !progress) importCard(card);
      });
      listen("card-unmounted", (e) => {
        // Card physically pulled (or ejected): drop any stale eject affordance.
        const dcim = e.payload?.dcim;
        if (ejectableCard && dcim && ejectableCard.dcim === dcim) ejectableCard = null;
      });
      listen("import-first-card-requested", async () => {
        const available = await invoke("find_cards");
        cards = available;
        if (available.length) importCard(available[0]);
      });
      listen("toggle-auto-import-requested", () => toggleAutoImport());
      listen("add-library-folder-requested", () => indexRoot());
      listen("app-error", (e) => {
        appMessage = e.payload.message ?? String(e.payload);
        setTimeout(() => (appMessage = ""), 5000);
      });
      let lastImportRefresh = 0;
      listen("import-progress", async (e) => {
        const payload = e.payload;
        progress = { verb: "import", ...payload };
        if (payload?.destDir && payload?.dest) {
          const list = importedByFolder.get(payload.destDir) ?? [];
          list.push(payload.dest);
          importedByFolder.set(payload.destDir, list);
        }
        if (payload?.destDir) {
          const now = Date.now();
          if (now - lastImportRefresh > 250) {
            lastImportRefresh = now;
            await refreshDirs(true);
            if (!curDir) {
              lastImportedFolder = payload.destDir;
              openDir(payload.destDir);
            } else if (curDir === payload.destDir) {
              try {
                const rawRows = await invoke("index_frames", { dir: curDir, minRating });
                const rows = (Array.isArray(rawRows) ? rawRows : []).filter(
                  (r) => r.name && !r.name.startsWith(".") && !r.name.startsWith("._")
                );
                frames = rows;
              } catch (err) {}
            }
          }
        }
      });
      listen("import-started", (e) => {
        progress = { verb: "import", done: 0, total: 1, current: "Démarrage..." };
        appMessage = `Import démarré...`;
        importedByFolder = new Map();
      });
      listen("import-finished", async (e) => {
        progress = null;
        appMessage = `Import terminé ✓`;
        const stats = e.payload;
        if (stats?.folders?.length) {
          const lastFolder = stats.folders[stats.folders.length - 1];
          lastImportedFolder = lastFolder;
          await refreshDirs();
          await openDir(lastFolder);
        }
        setTimeout(() => (appMessage = ""), 4000);
        // Walk-away AI cull: one folder at a time (not concurrently, so a
        // multi-day card doesn't hammer the vision API in parallel), only
        // for folders that actually received photos this run.
        if (aiCullEnabled && stats?.folders?.length) {
          for (const folder of stats.folders) {
            const folderPaths = importedByFolder.get(folder);
            if (folderPaths?.length) await triggerAiCull(folder, folderPaths);
          }
        }
      });
      listen("import-failed", (e) => {
        progress = null;
        appMessage = `Échec de l'import : ${e.payload.message}`;
        setTimeout(() => (appMessage = ""), 6000);
      });
      // AI cull toasts — reuses the same appMessage pattern as import/export
      // rather than a dedicated chip/modal (the flow is walk-away, no review
      // step to build UI for).
      listen("cull-started", (e) => {
        appMessage = `Culling IA · ${e.payload?.total ?? "?"} photos…`;
      });
      listen("cull-progress", (e) => {
        const p = e.payload;
        const phaseLabel = p.phase === "cloud" ? "analyse visuelle" : "tri local";
        appMessage = `Culling IA (${phaseLabel}) ${p.done}/${p.total}`;
        // Keeps the generic bottom HUD (`import-hud`, driven by `progress`)
        // ticking in step with the toast instead of freezing at the 0/N it
        // was set to when the run started.
        progress = { verb: "cull", done: p.done, total: p.total, current: phaseLabel };
      });
      listen("cull-finished", (e) => {
        const stats = e.payload;
        appMessage = `Culling IA ✓ ${stats.picked}/${stats.considered} conservés → ${stats.exported_to}`;
        setTimeout(() => (appMessage = ""), 6000);
      });
      listen("cull-failed", (e) => {
        appMessage = `Culling IA : ${e.payload.message}`;
        setTimeout(() => (appMessage = ""), 6000);
      });
      listen("export-progress", (e) => {
        progress = { verb: "export", ...e.payload };
        if (activeExportJobId) {
          updateQueueJob(activeExportJobId, {
            current: e.payload.current || "Finishing",
            done: e.payload.done,
            total: e.payload.total,
            phase: e.payload.phase || "Developing",
            status: e.payload.cancelled
              ? "cancelled"
              : e.payload.done === e.payload.total
                ? "completed"
                : "running",
          });
        }
        if (e.payload.done === e.payload.total || e.payload.cancelled) {
          setTimeout(() => { progress = null; }, 3000);
        }
      });
      listen("publish-progress", (e) =>
        (progress = { verb: e.payload.phase || "publication", ...e.payload }));
      const pollCards = async () => (cards = await invoke("find_cards"));
      pollCards();
      setInterval(pollCards, 5000);

      if (isTauri) {
        listen("dev-panel-ready", () => {
          sendDevStateToPanel();
        });
        listen("dev-panel-recipe-updated", (e) => {
          if (recipe) {
            if (e.payload && e.payload.key) {
              lastEditedKey = e.payload.key;
            }
            const merged = /** @type {Recipe} */ ({ ...recipe, ...e.payload.recipe });
            recipe = merged;
            // Mirror recipe.engine (a Rust engine id) into the UI selection.
            if (merged.engine === "rapid" || merged.engine === "spektra") {
              developEngine = merged.engine;
            } else if (merged.engine === "spektrafilm-rs") {
              developEngine = "spektra"; // legacy alias
            }
            edited(e.payload.transient);
          }
        });
        listen("dev-panel-engine-updated", (e) => {
          if (e.payload.engine === null) {
            clearDevelopment();
          } else {
            developEngine = e.payload.engine;
            if (recipe) {
              recipe.engine = developEngine === "rapid" ? "rapid" : "spektra";
            }
            edited(false);
          }
        });
        listen("dev-panel-caption-updated", (e) => {
          caption = e.payload.caption;
          captionEdited();
        });
        listen("dev-panel-export", () => {
          exportCurrent();
        });
        listen("dev-panel-export-desktop", () => {
          exportCurrent(""); // "" = the Desktop, regardless of the configured export folder
        });
        listen("dev-panel-export-vault", () => {
          exportToVault();
        });
        listen("dev-panel-reset", async () => {
          // RÉINITIALISER — back to the engine defaults, like Swift's
          // resetSettings; the sidecar re-saves through the normal edit path.
          if (recipe) {
            recipe = await invoke("default_recipe");
            edited();
          }
        });
        listen("dev-panel-export-settings-changed", (e) => {
          exportEdge = e.payload.exportEdge;
          exportBorder = e.payload.exportBorder;
          saveExportPrefs();
        });
        listen("dev-panel-choose-export-folder", () => {
          chooseExportFolder();
        });
        listen("dev-panel-open-in-editor", (e) => {
          openInEditor(e.payload.appPath);
        });
        listen("dev-panel-switch-mode", (e) => {
          switchMode(e.payload.mode);
        });
        // The dev panel is a separate OS window, so it steals keyboard focus:
        // shortcuts pressed there never reached the main window (g, arrows, …
        // did nothing until you clicked back). It forwards them here and we
        // replay them through the same onKey with a synthetic event.
        listen("dev-panel-key", (e) => {
          const p = e.payload || {};
          onKey(/** @type {any} */ ({
            key: p.key,
            metaKey: !!p.metaKey,
            ctrlKey: !!p.ctrlKey,
            shiftKey: !!p.shiftKey,
            target: { tagName: "" },
            preventDefault() {},
            stopPropagation() {},
          }));
        });
        listen("dev-panel-zoom", (e) => {
          cycleZoom(e.payload?.reverse);
        });
        listen("dev-panel-close", () => {
          layouts.dev.devPanel = false;
          saveLayouts();
        });
        listen("open-settings-requested", openSettings);
        listen("toggle-dev-panel-requested", toggleDevPanel);
        listen("toggle-preset-panel-requested", togglePresetPanel);
        listen("toggle-lut-panel-requested", toggleLutPanel);
        /** @type {Recipe | null} */ let savedRecipeBeforeHover = null;
        /** @type {string | null} */ let savedEngineBeforeHover = null;

        listen("preset-preview", (e) => {
          const payload = e.payload || {};
          const previewRecipe = payload.recipe;
          if (previewRecipe) {
            if (!savedRecipeBeforeHover && recipe) {
              savedRecipeBeforeHover = { ...recipe };
              savedEngineBeforeHover = developEngine;
            }
            recipe = { ...previewRecipe };
            developEngine = previewRecipe.engine === "rapid" ? "rapid" : "spektra";
            scheduleRender(PREVIEW_PX);
            sendDevStateToPanel();
          } else if (savedRecipeBeforeHover) {
            recipe = { ...savedRecipeBeforeHover };
            developEngine = savedEngineBeforeHover;
            savedRecipeBeforeHover = null;
            savedEngineBeforeHover = null;
            if (developEngine && developEngine !== "none") {
              scheduleRender(PREVIEW_PX);
            } else {
              clearDevelopment();
            }
            sendDevStateToPanel();
          }
        });

        listen("dev-panel-toggle-clipping", (e) => {
          const payload = e.payload || {};
          if (typeof payload.showClipping === "boolean") {
            showClipping = payload.showClipping;
          } else {
            showClipping = !showClipping;
          }
          sendDevStateToPanel();
        });

        // The Preset palette asks the main window to apply a saved recipe. The
        // main window owns the selection, so it resolves the target set here:
        // "selected" (modifier held) → all selected; "auto" → all selected when
        // several are, else just the current photo.
        listen("preset-apply", (e) => {
          savedRecipeBeforeHover = null;
          savedEngineBeforeHover = null;
          const payload = e.payload || {};
          const presetRecipe = payload.recipe;
          if (!presetRecipe) return;
          const selected = selectedFrames();
          const current = view[sel] ? [view[sel]] : selected;
          const targets =
            payload.scope === "selected"
              ? (selected.length ? selected : current)
              : (selected.length > 1 ? selected : current);
          applyRecipeToFrames(presetRecipe, targets);
        });
        listen("toggle-render-queue-requested", () => (queueOpen = !queueOpen));
        listen("toggle-shortcuts-requested", () => (shortcutsOpen = !shortcutsOpen));
        listen("menu-export-requested", exportCurrent);
        listen("menu-reset-develop-requested", async () => {
          if (recipe) {
            recipe = await invoke("default_recipe");
            edited();
          }
        });
        listen("menu-clear-develop-requested", clearDevelopment);
        listen("menu-enable-develop-requested", () => {
          if (currentMode !== "dev") switchMode("dev");
          else edited(false);
        });
        listen("menu-publish-requested", () => {
          if (view[sel]) developFromMenu(view[sel].path, true);
        });
      }

      const auto = await invoke("autoload_path") || await invoke("take_open_file");
      if (auto) {
        if (auto.endsWith("/") || !auto.includes(".")) openFolder(auto);
        else openPhoto(auto);
      } else {
        const savedDir = localStorage.getItem("reveal.lastDirectory");
        // Use the persisted path even when a slow or temporarily unavailable
        // network volume has not returned it in the sidebar index yet.
        const lastDir = savedDir || dirs[dirs.length - 1]?.dir;
        if (lastDir) openDir(lastDir, false);
      }
    })();
  });

  async function toggleAutoImport() {
    const prefs = await invoke("toggle_auto_import");
    autoImport = !!prefs.auto_import;
    appMessage = autoImport ? "import auto activé" : "import auto désactivé";
    setTimeout(() => (appMessage = ""), 2500);
  }

  function saveLayouts() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("reveal.modeLayouts", JSON.stringify(layouts));
    }
  }

  /**
   * @param {"cull" | "dev" | "story"} to
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  async function switchMode(to, { openDevPanel = true } = {}) {
    closePhotoMenu();
    if (to === "story" && !curDir && !folder) return;
    if (to !== "dev") zoomMode = "frame"; // always re-enter develop framed
    if (to === "dev" && openDevPanel) {
      // Develop always opens as a complete workspace. A deliberate entry ends
      // any lingering quick look, so the panel obeys its stored preference.
      spaceLook = false;
      if (currentMode !== "dev") {
        // Closing the panel only hides it for the current visit; the next D
        // restores it. Space (quick single-photo look) opts out of this.
        layouts.dev.devPanel = true;
        saveLayouts();
      }
    }
    currentMode = to;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("reveal.currentMode", to);
      if (curDir) {
        localStorage.setItem(`reveal.mode.${curDir}`, to);
      } else if (folder) {
        localStorage.setItem(`reveal.mode.${folder}`, to);
      }
    }

    // Sync focus state to Rust
    const currentFocus = layouts[currentMode].focus;
    if (isTauri) {
      invoke("set_focus", { enabled: currentFocus }).catch(() => {});
    }

    if (to === "dev") {
      if (view[sel] && (photoPath !== view[sel].path || !recipe)) {
        openPhoto(view[sel].path);
      } else if (useCanvas) {
        // Re-entering dev on the SAME photo: DevelopView was unmounted while
        // we were in the grid, which destroyed the <canvas> element and its
        // painted bitmap (canvasEl got rebound to a fresh, blank one). The
        // <img> path survives because its blob URL keeps the last frame; the
        // canvas (rapid engine) path doesn't, so it needs an explicit repaint.
        scheduleRender(PREVIEW_PX);
      }
    }
  }

  function cycleZoom(reverse = false) {
    if (currentMode !== "dev") return;
    const i = ZOOM_CYCLE.indexOf(zoomMode);
    const step = reverse ? -1 : 1;
    zoomMode = ZOOM_CYCLE[(i + step + ZOOM_CYCLE.length) % ZOOM_CYCLE.length];
  }

  // Drag-to-pan in the 100% ("actual") view: grab the photo and move it, like
  // Lightroom's loupe. We drive the scroll container directly, so trackpad
  // scrolling keeps working alongside the drag.
  let panning = $state(false);
  let panOrigin = { x: 0, y: 0, left: 0, top: 0 };

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onPhotoPointerDown(e) {
    if (zoomMode !== "actual" || e.button !== 0) return;
    const el = e.currentTarget;
    panning = true;
    panOrigin = { x: e.clientX, y: e.clientY, left: el.scrollLeft, top: el.scrollTop };
    el.setPointerCapture?.(e.pointerId);
    e.stopPropagation(); // don't let the app chrome start a window drag
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onPhotoPointerMove(e) {
    if (!panning) return;
    const el = e.currentTarget;
    el.scrollLeft = panOrigin.left - (e.clientX - panOrigin.x);
    el.scrollTop = panOrigin.top - (e.clientY - panOrigin.y);
  }

  /** @param {PointerEvent & { currentTarget: HTMLElement }} e */
  function onPhotoPointerUp(e) {
    if (!panning) return;
    panning = false;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
  }

  async function toggleFocusMode() {
    const targetFocus = !layouts[currentMode].focus;
    layouts[currentMode].focus = targetFocus;
    saveLayouts();
    if (isTauri) {
      await invoke("set_focus", { enabled: targetFocus }).catch(() => {});
    }
  }

  $effect(() => {
    if (!isTauri || typeof window === "undefined") return;

    /** @param {boolean} inside */
    const setPresence = (inside) => {
      pointerInside = inside;
      invoke("set_focus_window_presence", { windowId: "main", inside }).catch(() => {});
    };
    const onFocus = () => {
      if (pointerInside) setPresence(true);
    };
    const onBlur = () => setPresence(false);
    const onPointerEnter = () => setPresence(true);
    const onPointerLeave = () => setPresence(false);
    // pointerenter only fires on a boundary CROSSING — if the mouse was
    // already resting inside the window when this effect (re)subscribed
    // (e.g. switching into dev/fullscreen mode without moving the mouse),
    // no crossing ever happens and pointerInside stays stuck false. Any
    // real movement inside the window self-heals it.
    const onPointerMove = () => {
      if (!pointerInside) setPresence(true);
    };

    /** @param {DragEvent} e */
    const preventDragOver = (e) => e.preventDefault();
    /** @param {DragEvent} e */
    const preventDrop = (e) => e.preventDefault();

    window.addEventListener("pointerenter", onPointerEnter);
    window.addEventListener("pointerleave", onPointerLeave);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    window.addEventListener("dragover", preventDragOver, false);
    window.addEventListener("drop", preventDrop, false);
    return () => {
      window.removeEventListener("pointerenter", onPointerEnter);
      window.removeEventListener("pointerleave", onPointerLeave);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("dragover", preventDragOver, false);
      window.removeEventListener("drop", preventDrop, false);
      setPresence(false);
    };
  });

  function sendDevStateToPanel() {
    if (isTauri && currentMode === "dev") {
      emit("main-dev-state", {
        photoPath: photoPath,
        picked: picked,
        rating: frames.find((f) => f.path === photoPath)?.rating ?? 0,
        recipe: recipe ? { ...recipe } : null,
        developEngine,
        renderMs: renderMs,
        status: status,
        installedEditors: installedEditors,
        exportEdge: exportEdge,
        exportBorder: exportBorder,
        exportFolder: exportFolder,
        films: films,
        papers: papers,
        luts: luts,
        engines: engines,
        caption: caption,
        showClipping: showClipping
      }).catch(() => {});
    }
  }

  // The Develop workspace is a set of floating palette windows — the Develop
  // panel plus the LUT and Preset palettes. Each is its own always-on-top macOS
  // window (label + route + a `layouts.dev` flag), opened together on entering
  // Develop and independently closeable / re-openable from the Window menu. All
  // three receive `main-dev-state` and edit the one recipe the main owns.
  const PALETTE_SPECS = [
    { label: "develop-panel", url: "/dev-panel",    flag: "devPanel",    width: 320, height: 850 },
  ];

  /** @param {string} label */
  function paletteTitle(label) {
    if (label === "develop-panel") return picked ? `${picked} — Chambre Noire` : "Chambre Noire";
    if (label === "lut-panel") return "LUTs";
    if (label === "preset-panel") return "Presets";
    return "Reveal";
  }

  /**
   * Beside the main window when there's room (right, then left); a
   * different monitor if neither side fits on the main window's own
   * monitor; the monitor's own right edge as a last resort when there's
   * truly nowhere else. Palettes cascade by index so they don't spawn
   * perfectly stacked.
   *
   * Called on EVERY show, not just window creation — recomputing once and
   * letting `tauri-plugin-window-state` persist that position forever meant
   * the panel drifted on top of the main window as soon as it moved/resized
   * from wherever it was when the panel was first created (reproduced
   * 2026-08-04).
   * @param {number} index
   * @param {number} panelWidth
   */
  async function computePalettePosition(index, panelWidth) {
    const GAP = 12;
    const CASCADE = 34;
    try {
      const main = getCurrentWindow();
      const factor = await main.scaleFactor();
      const outer = await main.outerPosition();
      const size = await main.outerSize();
      const mainMon = await currentMonitor();
      if (!mainMon?.position || !mainMon?.size) return null;

      const mainX = outer.x / factor;
      const mainY = outer.y / factor;
      const mainW = size.width / factor;
      const monX = mainMon.position.x / factor;
      const monW = mainMon.size.width / factor;

      const rightX = Math.round(mainX + mainW) + GAP;
      const fitsRight = rightX + panelWidth <= monX + monW;
      const leftX = Math.round(mainX) - GAP - panelWidth;
      const fitsLeft = leftX >= monX;

      let baseX;
      if (fitsRight) {
        baseX = rightX;
      } else if (fitsLeft) {
        baseX = leftX;
      } else {
        const monitors = await availableMonitors();
        const other = monitors.find(
          (m) => m.position.x !== mainMon.position.x || m.position.y !== mainMon.position.y
        );
        if (other) {
          const oX = other.position.x / factor;
          const oY = other.position.y / factor;
          return { x: Math.round(oX + GAP) + index * CASCADE, y: Math.round(oY + GAP) + index * CASCADE };
        }
        // Truly nowhere else on a single monitor with a wide main window —
        // some overlap is unavoidable; at least stay predictable.
        baseX = Math.round(monX + monW - panelWidth);
      }
      return { x: baseX + index * CASCADE, y: Math.round(mainY) + index * CASCADE };
    } catch (_) {
      return null;
    }
  }

  /**
   * @param {{ label: string, url: string, flag: string, width: number, height: number }} spec
   * @param {number} index
   */
  async function syncPaletteWindow(spec, index) {
    if (!isTauri) return;
    try {
      let win = await WebviewWindow.getByLabel(spec.label);
      if (currentMode === "dev" && layouts.dev[spec.flag] && !spaceLook) {
        const pos = await computePalettePosition(index, spec.width);
        if (!win) {
          win = new WebviewWindow(spec.label, {
            url: spec.url,
            title: paletteTitle(spec.label),
            width: spec.width,
            height: spec.height,
            x: pos?.x,
            y: pos?.y,
            resizable: true,
            // A real child window (macOS `addChildWindow:`) stacks above the
            // main window and steps back with it when Reveal loses focus.
            // `alwaysOnTop` instead sets a system-wide floating window
            // level, which floats this panel above every OTHER app too —
            // reproduced 2026-08-02, the panel stayed pinned over an
            // unrelated app after switching away from Reveal.
            parent: getCurrentWindow(),
            titleBarStyle: "overlay",
            hiddenTitle: true
          });
          win.once("tauri://created", () => {
            setTimeout(sendDevStateToPanel, 400);
          });
          win.once("tauri://close-requested", () => {
            layouts.dev[spec.flag] = false;
            saveLayouts();
          });
        } else {
          if (pos) await win.setPosition(new LogicalPosition(pos.x, pos.y));
          await win.show();
          // Only the Develop panel grabs focus on show, so three palettes don't
          // fight over it when entering Develop.
          if (index === 0) await win.setFocus();
          sendDevStateToPanel();
        }
      } else {
        if (win) {
          await win.close();
        }
      }
    } catch (e) {
      // Surface to Rust stderr (app.html ships console.error → reveal://log)
      // so a missing permission or failed spawn isn't silent again.
      console.error("syncPaletteWindow:", spec.label, e);
    }
  }

  function syncPalettes() {
    PALETTE_SPECS.forEach((spec, i) => syncPaletteWindow(spec, i));
  }

  // Kept for the callers that specifically re-sync the Develop panel.
  async function syncDevPanelWindow() {
    await syncPaletteWindow(PALETTE_SPECS[0], 0);
  }

  $effect(() => {
    if (currentMode === "dev" && isTauri) {
      // Establish dependency on Svelte reactive variables
      const trigger = [photoPath, picked, recipe, developEngine, renderMs, status, installedEditors, exportEdge, exportBorder, films, papers, luts, caption, currentRating];
      sendDevStateToPanel();
    }
  });

  $effect(() => {
    if (isTauri) {
      // Reactively sync every palette when mode, any palette flag, or quick-look
      // toggles. spaceLook suppresses all palettes so quick-look stays clean.
      const trigger = [currentMode, layouts.dev.devPanel, spaceLook];
      syncPalettes();
    }
  });

  function toggleSidebar() {
    closeSidebarPeek();
    layouts[currentMode].sidebar = !layouts[currentMode].sidebar;
    saveLayouts();
  }

  function toggleDevPanel() {
    // ⇧D during a quick look just reveals the panel the look was suppressing,
    // rather than toggling the stored preference off.
    if (spaceLook && layouts[currentMode].devPanel) {
      spaceLook = false;
      return;
    }
    spaceLook = false;
    layouts[currentMode].devPanel = !layouts[currentMode].devPanel;
    saveLayouts();
  }

  // The Preset / LUT palettes toggle independently and are remembered; the
  // Window-menu items and any future shortcuts route through these.
  function togglePresetPanel() {
    // Legacy: used to toggle a standalone window. Can be repurposed later for Dev Tab switching.
  }

  function toggleLutPanel() {
    // Legacy: used to toggle a standalone window. Can be repurposed later for Dev Tab switching.
  }

  async function toggleAppearance() {
    try {
      await invoke("toggle_system_appearance");
    } catch (e) {
      appMessage = `apparence: ${e}`;
      setTimeout(() => (appMessage = ""), 5000);
    }
  }

  async function hideWindow() {
    await invoke("hide_contact_sheet");
  }

  // ---- index --------------------------------------------------------------
  // `light` skips the story-dot probe — one fs read per folder, too heavy to
  // repeat mid-scan over the NFS mount.
  async function refreshDirs(light = false) {
    try {
      const [r, d] = await invoke("index_dirs");
      roots = r; // index_dirs now returns the full root set
      dirs = d;
      if (!light) refreshStoryDirs();
      return true;
    } catch (error) {
      /** @type {RevealWindow} */ (window).__log?.(`folder tree unavailable: ${error}`);
      return false;
    }
  }

  // The sidebar's red "cette journée a une histoire" dots.
  async function refreshStoryDirs() {
    if (!isTauri || !dirs.length) return;
    try {
      storyDirs = new Set(await invoke("story_dirs", { dirs: dirs.map((d) => d.dir) }));
    } catch (e) {}
    // Refresh the story-notes lists too — pin/story changes flow through here.
    await refreshStoryNotes();
  }

  // Load every folder's story note metadata and split into the pinned list
  // (sorted by pinned-at descending — newest/newest-pinned at top) and the
  // recent list (mtime desc, top 12). Pinned notes may also appear in recent.
  async function refreshStoryNotes() {
    if (!isTauri || !dirs.length) {
      pinnedStories = [];
      recentStories = [];
      return;
    }
    /** @type {any[]} */
    let all = [];
    try {
      all = await invoke("list_story_notes", { dirs: dirs.map((d) => d.dir) });
    } catch (e) {
      pinnedStories = [];
      recentStories = [];
      return;
    }
    pinnedStories = all
      .filter((n) => n.pinned)
      // Descending by pinned-at: newest timestamp = top. reorderPinned stamps
      // index 0 with `now` and steps down 60s per index, so a descending sort
      // reproduces the displayed order; a new pin (now) floats to the top.
      .sort((a, b) => (b.pinnedAt ?? "").localeCompare(a.pinnedAt ?? ""));
    recentStories = [...all].sort((a, b) => b.mtime - a.mtime).slice(0, 12);
  }

  function saveGridPrefs() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        "reveal.grid",
        JSON.stringify({ cols, marginScale, cellAspect, fillCells, sortDesc }),
      );
    }
  }

  // What the grid actually shows: the backend rows, optionally narrowed to
  // the story set (Collection Rapide), in the chosen name order.
  const view = $derived.by(() => {
    let rows = frames;
    if (filterStory) rows = rows.filter((f) => storySet.has(stem(f.name)));
    // Ascending = the backend's ORDER BY capture_at, name. Descending flips
    // on the same key — capture date first, filename as the tiebreak.
    if (sortDesc) {
      rows = [...rows].sort(
        (a, b) => Number(b.capture_at || 0) - Number(a.capture_at || 0) || b.name.localeCompare(a.name),
      );
    }
    return rows;
  });

  $effect(() => {
    if (sel >= view.length) {
      sel = Math.max(0, view.length - 1);
      selectOnly(sel);
    }
  });

  /** @param {number} index */
  function selectOnly(index) {
    const frame = view[index];
    selectedPaths = new Set(frame ? [frame.path] : []);
    selectionAnchor = index;
  }

  /**
   * @param {number} index
   * @param {MouseEvent} [event]
   */
  function selectGridItem(index, event) {
    const frame = view[index];
    if (!frame) return;

    if (event?.shiftKey) {
      const from = Math.min(selectionAnchor, index);
      const to = Math.max(selectionAnchor, index);
      const next = event.metaKey || event.ctrlKey ? new Set(selectedPaths) : new Set();
      for (let i = from; i <= to; i += 1) next.add(view[i].path);
      selectedPaths = next;
    } else if (event?.metaKey || event?.ctrlKey) {
      const next = new Set(selectedPaths);
      if (next.has(frame.path)) next.delete(frame.path);
      else next.add(frame.path);
      selectedPaths = next;
      selectionAnchor = index;
    } else {
      selectOnly(index);
    }
    sel = index;
  }

  function selectedFrames() {
    const selected = view.filter((frame) => selectedPaths.has(frame.path));
    return selected.length ? selected : (view[sel] ? [view[sel]] : []);
  }

  /**
   * @param {number} index
   * @param {MouseEvent} event
   */
  function openPhotoMenu(index, event) {
    event.preventDefault();
    const frame = view[index];
    if (!frame) return;
    if (!selectedPaths.has(frame.path)) selectOnly(index);
    sel = index;
    const menuWidth = 230;
    const menuHeight = 360;
    photoMenu = {
      frame,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - menuWidth - 12)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - menuHeight - 12)),
      submenuLeft: event.clientX > window.innerWidth - (menuWidth * 2) - 20,
    };
  }

  function closePhotoMenu() {
    photoMenu = null;
  }

  async function openSettings() {
    if (isTauri) {
      const saved = await invoke("load_preferences");
      preferences = { ...preferences, ...saved, export_folder: saved.export_folder ?? exportFolder };
    }
    settingsOpen = true;
  }

  async function saveSettings() {
    if (isTauri) await invoke("save_preferences", { preferences });
    exportFolder = preferences.export_folder ?? "";
    saveExportPrefs();
    aiCullEnabled = !!preferences.ai_cull_enabled;
    aiCullTarget = Number(preferences.ai_cull_target) || 24;
    settingsOpen = false;
  }

  // The sidebar's Garden account row — sign-in verifies the pasted key
  // against /api/me and stores it; errors bubble to the popover.
  /** @param {string} apiKey */
  async function gardenSignIn(apiKey) {
    gardenAccount = await invoke("garden_sign_in", { apiKey });
  }

  async function gardenSignOut() {
    gardenAccount = await invoke("garden_sign_out");
  }

  /** @param {string} url */
  async function openUrl(url) {
    if (isTauri) await invoke("open_path", { path: url });
    else window.open(url, "_blank");
  }

  /** @param {string} key */
  async function choosePreferenceFolder(key) {
    if (!isTauri) return;
    const path = await invoke("pick_folder");
    if (path) preferences = { ...preferences, [key]: path };
  }

  async function closeMainWindow() {
    try {
      await getCurrentWindow().close();
    } catch (error) {
      console.error("closeMainWindow:", error);
    }
  }

  async function minimizeMainWindow() {
    try {
      await getCurrentWindow().minimize();
    } catch (error) {
      console.error("minimizeMainWindow:", error);
    }
  }

  async function zoomMainWindow() {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch (error) {
      console.error("zoomMainWindow:", error);
    }

  }

  /** @param {MouseEvent} event */
  async function startWindowDrag(event) {
    if (!isTauri || event.button !== 0) return;
    const target = event.target;
    if (
      target instanceof Element &&
      target.closest(
        "button, input, select, textarea, a, nav, [role='button'], [role='menu'], [contenteditable='true'], [draggable='true'], .cell, .photo-cell, .roll-cell, .frame, .gap, .composer, .surface, .roll, [role='listitem'], .sidebar-peek, .popover, .settings-overlay, .photo-mat",
      )
    ) {
      return;
    }
    try {
      await getCurrentWindow().startDragging();
    } catch (error) {
      console.error("startWindowDrag:", error);
    }
  }

  /** @param {string} path */
  async function prepareFullscreenFrame(path) {
      const request = ++fullscreenRequest;
      const frame = frames.find((item) => item.path === path);
      const immediate =
        currentMode === "dev" && photoPath === path && imgUrl
          ? imgUrl
          : thumbUrl(path, frame?.previewVersion ?? 0);
      if (fullscreenUrl?.startsWith("blob:") && fullscreenUrl !== imgUrl) {
        URL.revokeObjectURL(fullscreenUrl);
      }
      fullscreenUrl = immediate;
      if (currentMode === "dev" && photoPath === path && imgUrl?.startsWith("blob:")) return;

      try {
        const sidecar = await invoke("load_sidecar", { path });
        if (!sidecar?.engine_settings) return;
        const bytes = await invoke("develop_preview", {
          path,
          recipe: sidecar.engine_settings,
          maxPx: 2560,
        });
        if (!fullscreen || request !== fullscreenRequest || view[sel]?.path !== path) return;
        if (fullscreenUrl?.startsWith("blob:") && fullscreenUrl !== imgUrl) {
          URL.revokeObjectURL(fullscreenUrl);
        }
        fullscreenUrl = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
      } catch (error) {
        console.error("prepareFullscreenFrame:", error);
      }
  }

  async function enterFullscreen() {
      const frame = view[sel];
      if (!frame || fullscreen) return;
      fullscreen = true;
      prepareFullscreenFrame(frame.path);
      if (isTauri) {
        const panel = await WebviewWindow.getByLabel("develop-panel");
        if (panel) await panel.hide();
        // Lightroom-style borderless fullscreen (fill the display in place, no
        // Spaces animation) — NOT the macOS native setFullscreen.
        await invoke("set_simple_fullscreen", { enabled: true });
      }
  }

  async function exitFullscreen() {
      if (!fullscreen) return;
      fullscreen = false;
      fullscreenRequest += 1;
      if (fullscreenUrl?.startsWith("blob:") && fullscreenUrl !== imgUrl) {
        URL.revokeObjectURL(fullscreenUrl);
      }
      fullscreenUrl = null;
      if (isTauri) {
        await invoke("set_simple_fullscreen", { enabled: false });
        if (currentMode === "dev" && layouts.dev.devPanel) await syncDevPanelWindow();
      }
  }

  function toggleFullscreen() {
    if (fullscreen) exitFullscreen();
    else enterFullscreen();
  }

  /** @param {HTMLElement} node */
  function trackDevelopViewport(node) {
    const observer = new ResizeObserver(([entry]) => {
      developViewport = {
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      };
    });
    observer.observe(node);
    return {
      destroy() {
        observer.disconnect();
      },
    };
  }

  /** @param {string} path */
  async function revealPhotoInFinder(path) {
    closePhotoMenu();
    try {
      await invoke("reveal_in_finder", { path });
    } catch (error) {
      appMessage = `Could not reveal photo: ${error}`;
    }
  }

  /** @param {string} path */
  async function openPhotoPreview(path) {
    closePhotoMenu();
    try {
      await invoke("open_path", { path });
    } catch (error) {
      appMessage = `Could not open photo: ${error}`;
    }
  }

  /**
   * @param {string} path
   * @param {string} appPath
   */
  async function openPhotoInEditor(path, appPath) {
    closePhotoMenu();
    try {
      await invoke("open_in_editor", { filePath: path, appPath });
    } catch (error) {
      appMessage = `Could not open editor: ${error}`;
    }
  }

  /**
   * @param {string} path
   * @param {boolean} toVault
   */
  async function developFromMenu(path, toVault) {
    closePhotoMenu();
    try {
      const sidecar = await invoke("load_sidecar", { path });
      if (!sidecar?.engine_settings) {
        appMessage = "No development engine is active for this photo";
        setTimeout(() => (appMessage = ""), 3000);
        return;
      }
      if (toVault) {
        appMessage = await invoke("publish_photo", { path });
        return;
      }
      progress = { verb: "Developing", done: 0, total: 1, current: path.split("/").pop() };
      await invoke("export_photo", {
        path,
        recipe: sidecar.engine_settings,
        destDir: exportFolder,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      progress = { ...progress, done: 1 };
    } catch (error) {
      appMessage = `Development failed: ${error}`;
    } finally {
      if (progress) {
        setTimeout(() => {
          progress = null;
        }, 1200);
      }
    }
  }

  async function indexRoot() {
    const path = await invoke("pick_folder");
    if (!path) return;
    scanning = true;
    try {
      await invoke("scan_root", { path });
      await refreshDirs();
      if (dirs.length) openDir(dirs[dirs.length - 1].dir);
    } finally {
      scanning = false;
    }
  }

  async function rescan() {
    if (!roots.length) return;
    scanning = true;
    try {
      for (const r of roots) await invoke("scan_root", { path: r });
      await refreshDirs();
      if (curDir) openDir(curDir);
      else if (dirs.length) openDir(dirs[dirs.length - 1].dir);
    } finally {
      scanning = false;
    }
  }

  /** @param {string} path */
  async function rescanDir(path) {
    if (!path || scanning) return;
    scanning = true;
    appMessage = `Reindexing ${path.split("/").pop()}…`;
    try {
      await invoke(roots.includes(path) ? "scan_root" : "scan_folder", { path });
      await refreshDirs();
      if (curDir?.startsWith(path)) await openDir(curDir);
      appMessage = `Reindexed ${path.split("/").pop()}`;
    } catch (error) {
      appMessage = `Could not reindex folder: ${error}`;
    } finally {
      scanning = false;
      setTimeout(() => {
        if (appMessage.startsWith("Reindexed ")) appMessage = "";
      }, 2200);
    }
  }

  /** @param {string} path */
  async function revealDir(path) {
    try {
      await invoke("open_path", { path });
    } catch (error) {
      appMessage = `Could not open folder: ${error}`;
    }
  }

  // Set the import destination folder (the sidebar's context-menu action).
  // The backend persists it and emits shell-prefs-changed, which updates
  // `importDir` reactively — so the sidebar's accent follows without a
  // manual refresh. A toast confirms the choice since there's no other UI.
  /** @param {string} path */
  async function setImportDir(path) {
    try {
      await invoke("set_import_dir", { path });
      const name = path?.split("/").pop() || "(racine)";
      appMessage = `Import → ${name}`;
      setTimeout(() => {
        if (appMessage.startsWith("Import → ")) appMessage = "";
      }, 2500);
    } catch (error) {
      appMessage = `Could not set import folder: ${error}`;
    }
  }

  // ---- drag a photo onto a sidebar folder to move it there -------------------
  // The dragged frame carries its path(s) on the drag session; a folder row in
  // the sidebar reads them on drop and calls movePhotos. Dragging a photo that's
  // part of the current selection moves the whole selection.
  /**
   * @param {string} path
   * @param {DragEvent} event
   */
  function onPhotoDragStart(path, event) {
    if (!event.dataTransfer) return;
    const paths =
      selectedPaths.has(path) && selectedPaths.size > 1 ? [...selectedPaths] : [path];
    const payload = JSON.stringify(paths);
    event.dataTransfer.setData("application/x-reveal-photos", payload);
    event.dataTransfer.setData("text/plain", payload);
    event.dataTransfer.effectAllowed = "move";

    const target = /** @type {HTMLElement} */ (event.currentTarget || event.target);
    const img = target?.querySelector?.("img") || (event.target instanceof HTMLImageElement ? event.target : null);
    if (img && img instanceof HTMLImageElement && event.dataTransfer.setDragImage) {
      event.dataTransfer.setDragImage(img, Math.round(img.offsetWidth / 2), Math.round(img.offsetHeight / 2));
    }
  }

  /**
   * @param {string[]} paths
   * @param {string} destDir
   */
  async function movePhotos(paths, destDir) {
    if (!destDir || !paths?.length) return;
    /** @param {string} p */
    const parentOf = (p) => p.slice(0, p.lastIndexOf("/"));
    const toMove = paths.filter((p) => parentOf(p) !== destDir);
    if (!toMove.length) {
      appMessage = "déjà dans ce dossier";
      setTimeout(() => (appMessage = ""), 2500);
      return;
    }
    const srcDirs = new Set(toMove.map(parentOf));
    progress = { verb: "move", done: 0, total: toMove.length, current: "" };
    let moved = 0;
    const errors = [];
    for (const p of toMove) {
      try {
        await invoke("move_photo", { path: p, destDir });
        moved += 1;
        progress = { verb: "move", done: moved, total: toMove.length, current: p.split("/").pop() };
      } catch (e) {
        errors.push(`${p.split("/").pop()} : ${e}`);
      }
    }
    // Reconcile the index — the destination gains frames, each source loses them.
    try {
      await invoke("scan_folder", { path: destDir });
      for (const d of srcDirs) await invoke("scan_folder", { path: d });
    } catch (_) {}
    await refreshDirs();
    if (curDir) await openDir(curDir);
    selectedPaths = new Set();
    progress = null;
    const destName = destDir.split("/").pop();
    appMessage = errors.length
      ? `${moved} déplacée${moved > 1 ? "s" : ""} · ${errors.length} échec${errors.length > 1 ? "s" : ""}`
      : `${moved} photo${moved > 1 ? "s" : ""} déplacée${moved > 1 ? "s" : ""} → ${destName}`;
    if (errors.length) console.warn("move errors:", errors);
    return;
  }

  /** @param {string} destDir */
  function moveSelectedPhotosToDir(destDir) {
    const paths = [...selectedPaths];
    if (!paths.length && view[sel]) paths.push(view[sel].path);
    if (paths.length) movePhotos(paths, destDir);
  }

  // ---- folder-level file management (rename / create / move a folder) ------
  // All three touch the filesystem directly, then reconcile via `scan_root`
  // rather than a scoped `scan_folder`: the folder's OLD path no longer
  // exists after a rename/move, and reveal-index's walker treats an
  // unreadable root as "saw nothing under it" — which prunes every row under
  // that root unconditionally, not just a safe no-op. Scoping to the vanished
  // old path specifically would still correctly prune it (same mechanism),
  // but `scan_root` also picks up the folder's frames appearing fresh under
  // its new path in the same pass, and walks from a root that's guaranteed
  // to still exist. Same cost as clicking "Force Reindex" — not a new class
  // of operation, just triggered automatically here.
  async function reconcileAfterFileOp() {
    if (!root) return;
    try {
      await invoke("scan_root", { path: root });
    } catch (_) {}
    await refreshDirs();
  }

  /**
   * @param {string} path
   * @param {string} newName
   */
  async function renameDir(path, newName) {
    try {
      const newPath = await invoke("rename_dir", { path, newName });
      await reconcileAfterFileOp();
      if (curDir === path) {
        await openDir(newPath);
      } else if (curDir && curDir.startsWith(path + "/")) {
        await openDir(newPath + curDir.slice(path.length));
      }
      appMessage = `Renommé → ${newName}`;
    } catch (e) {
      appMessage = `Renommage échoué : ${e}`;
    }
    setTimeout(() => (appMessage = ""), 3000);
  }

  /**
   * @param {string} parentDir
   * @param {string} name
   */
  async function createFolder(parentDir, name) {
    try {
      const abs = await invoke("create_dir", { parentDir, name });
      appMessage = `Dossier créé : ${name}`;
      setTimeout(() => (appMessage = ""), 2500);
      return abs;
    } catch (e) {
      appMessage = `Création échouée : ${e}`;
      setTimeout(() => (appMessage = ""), 4000);
      return null;
    }
  }

  /**
   * @param {string} path
   * @param {string} destParentDir
   */
  async function moveDir(path, destParentDir) {
    const name = path.split("/").pop();
    try {
      const newPath = await invoke("move_dir", { path, destParentDir });
      await reconcileAfterFileOp();
      if (curDir === path) {
        await openDir(newPath);
      } else if (curDir && curDir.startsWith(path + "/")) {
        await openDir(newPath + curDir.slice(path.length));
      }
      appMessage = `${name} déplacé`;
    } catch (e) {
      appMessage = `Déplacement échoué : ${e}`;
    }
    setTimeout(() => (appMessage = ""), 3000);
  }

  let debug = $state("");
  // File over app: stamp each frame's thumb version with its .preview.jpg
  // mtime, so an external edit to a sidecar busts the grid cache on next load.
  /** @param {Frame[]} rows */
  async function withPreviewVersions(rows) {
    if (!isTauri || !Array.isArray(rows) || !rows.length) return rows;
    const batchSize = Math.min(rows.length, 500);
    try {
      const versions = await invoke("preview_versions", {
        paths: rows.slice(0, batchSize).map((r) => r.path)
      });
      for (let i = 0; i < versions.length; i++) rows[i].previewVersion = versions[i] ?? 0;
    } catch (e) {
      // Non-fatal — thumbs just fall back to version 0.
    }
    return rows;
  }

  /**
   * @param {string} dir
   * @param {boolean} [restoreMode]
   */
  async function openDir(dir, restoreMode = true) {
    // NOTE: the index stores dirs in the canonical firmlink form
    // (/System/Volumes/Data/mnt/…) — pass paths through verbatim; any
    // "normalization" to the short alias breaks the exact-match query.
    curDir = dir;
    folder = null;
    loading = true; // suppress the empty-state splash until the new frames land
    frames = []; // Immediately unmount previous grid cells to cancel pending thumbnail requests
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("reveal.lastDirectory", dir);
    }
    // Folder switch always starts with the full contact sheet.
    minRating = 0;
    filterStory = false;
    debug = "invoke…";
    /** @type {RevealWindow} */ (window).__log?.(`openDir start dir=${dir} minRating=${JSON.stringify(minRating)}`);
    try {
      const rawRows = await invoke("index_frames", { dir, minRating });
      const rows = (Array.isArray(rawRows) ? rawRows : []).filter(r => r.name && !r.name.startsWith('.') && !r.name.startsWith('._'));
      debug = `reçu ${rows.length}`;
      frames = rows;
      if (frames.length > 500 && layout === "masonry") {
        layout = "uniform"; // Fallback to virtualized grid to prevent memory/CPU explosion
      }
      withPreviewVersions(rows).then((updated) => {
        // Fresh array ref: `updated` is the same array we mutated in place, and
        // a raw $state only reacts to an identity change.
        if (curDir === dir && Array.isArray(updated)) frames = [...updated];
      });
    } catch (e) {
      debug = `échec: ${e}`;
      /** @type {RevealWindow} */ (window).__log?.(`openDir échec: ${e}`);
    }
    sel = 0;
    selectOnly(0);
    await switchMode("cull");
    refreshStory();
    const savedScroll = scrollOffsets[dir] || 0;
    setTimeout(() => {
      currentScrollTop = savedScroll;
    }, 50);
    loading = false; // frames have settled — re-enable the empty-state for genuinely empty folders
  }

  /** @param {string} dir */
  function dirLabel(dir) {
    // Label relative to whichever root owns this dir (multi-root).
    const owner = roots.find((r) => dir === r || dir.startsWith(r + "/"));
    return owner
      ? dir.slice(owner.length).replace(/^\//, "") || owner.split("/").pop()
      : dir.split("/").pop();
  }

  // ---- stories ---------------------------------------------------------------
  const gridDir = () => curDir ?? folder;
  /** @param {string} name */
  const stem = (name) => name.replace(/\.[^.]+$/, "");

  async function loadCatalogNote() {
    if (!root) return;
    catalogContent = await invoke("load_catalog_note", { root });
  }

  async function saveCatalogNote() {
    if (!root) return;
    await invoke("save_catalog_note", { root, content: catalogContent });
  }

  $effect(() => {
    if (root) {
      loadCatalogNote();
    }
  });

  async function loadStory() {
    const d = gridDir();
    if (!d) return;
    storyContent = await invoke("load_story_note", { dir: d });
  }

  // The WYSIWYG composer's persist hook: write its serialized note to disk and
  // refresh the story set (film-roll markers + sidebar dots). Deliberately does
  // NOT touch `storyContent` — that stays the last real load, so the composer
  // (which owns the blocks this session) never re-inits from its own output.
  /** @param {string} content */
  async function saveStoryContent(content) {
    const d = gridDir();
    if (!d) return;
    try {
      await invoke("save_story_note", { dir: d, content });
    } catch (e) {
      // The Rust side verifies every write (read-back byte-compare) before
      // replacing the note on disk — see story.rs `StoryNote::save`, added
      // after an NFS write silently dropped part of a note's content while
      // reporting success. A rejection here means that check just caught a
      // bad write and left the ON-DISK note untouched (the safe outcome),
      // but StoryComposer's `onSave(c)` call is fire-and-forget — without
      // this catch, that would be a silent, invisible failure: the edit
      // never reaches disk and nothing tells you.
      appMessage = `Échec de sauvegarde de l'histoire : ${e}`;
      setTimeout(() => (appMessage = ""), 8000);
      return;
    }
    storySet = new Set(await invoke("story_stems", { dir: d }));
    refreshStoryDirs();
  }

  async function refreshStory() {
    const d = gridDir();
    storySet = new Set(d ? await invoke("story_stems", { dir: d }) : []);
    if (d) {
      await loadStory();
    }
  }

  async function loadExternalEditors() {
    if (isTauri) {
      installedEditors = await invoke("list_external_editors");
    }
  }

  /** @param {string} appPath */
  async function openInEditor(appPath) {
    if (!appPath || !view[sel]) return;
    try {
      await invoke("open_in_editor", { filePath: view[sel].path, appPath });
      status = "Ouvert avec succès ✓";
      setTimeout(() => (status = status === "Ouvert avec succès ✓" ? "" : status), 2000);
    } catch (e) {
      status = `erreur : ${e}`;
    }
  }

  function toggleLayout() {
    if (layout === "uniform" && frames.length > 500) {
      appMessage = "Trop d'images pour le mode maçonnerie (>500)";
      setTimeout(() => (appMessage = ""), 4000);
      return;
    }
    layout = layout === "uniform" ? "masonry" : "uniform";
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("reveal.layout", layout);
    }
  }

  /**
   * @param {number | undefined} v
   * @param {number} min
   * @param {number} max
   */
  const pct = (v, min, max) => `${((Number(v ?? 0) - min) / (max - min)) * 100}%`;

  /** @param {string} path */
  async function toggleStoryWithPath(path) {
    const d = gridDir();
    if (!path || !d) return;
    storySet = new Set(await invoke("story_toggle", { dir: d, path }));
    await loadStory();
    refreshStoryDirs();
  }

  async function toggleStory() {
    const f = view[sel];
    if (!f) return;
    await toggleStoryWithPath(f.path);
  }

  // Pin/unpin a story note. Pin state lives in frontmatter (file-first); a new
  // pin stamps now so it floats to the top of the pinned list.
  /**
   * @param {string} notePath
   * @param {boolean} pinned
   */
  async function setStoryPinned(notePath, pinned) {
    const pinnedAt = pinned ? new Date().toISOString() : null;
    const folder = notePath.substring(0, notePath.lastIndexOf("/"));
    try {
      await invoke("story_set_pinned", { dir: folder, pinned, pinnedAt });
    } catch (e) {}
    await refreshStoryNotes();
  }

  // Rewrite pinned-at across every pinned note so the chronological sort in
  // refreshStoryNotes reproduces the displayed order: index 0 = newest (now),
  // stride 60s. See spec "Pin ordering invariant" + "Reorder algorithm".
  /**
   * @param {number} fromIndex
   * @param {number} toIndex
   */
  async function reorderPinned(fromIndex, toIndex) {
    const reordered = [...pinnedStories];
    const [moved] = reordered.splice(fromIndex, 1);
    reordered.splice(toIndex, 0, moved);
    const now = Date.now();
    for (let i = 0; i < reordered.length; i++) {
      const pinnedAt = new Date(now - i * 60_000).toISOString();
      const folder = reordered[i].notePath.substring(
        0,
        reordered[i].notePath.lastIndexOf("/"),
      );
      try {
        await invoke("story_set_pinned", { dir: folder, pinned: true, pinnedAt });
      } catch (e) {}
    }
    await refreshStoryNotes();
  }

  async function publishStory() {
    const d = gridDir();
    if (!d || !storySet.size) return;
    liveUrl = null;
    progress = { verb: "publication", done: 0, total: storySet.size, current: "" };
    try {
      liveUrl = await invoke("publish_story", { dir: d, dryRun: false });
      status = "publié ✓";
      setTimeout(() => (status = status === "publié ✓" ? "" : status), 2000);
    } catch (e) {
      status = `erreur : ${e}`;
    } finally {
      progress = null;
    }
  }

  async function exportLocalStory() {
    const d = gridDir();
    if (!d || !storySet.size) return;
    const dest = await invoke("pick_folder");
    if (!dest) return;
    liveUrl = null;
    progress = { verb: "export", done: 0, total: storySet.size, current: "" };
    try {
      await invoke("export_local_story", {
        dir: d,
        dest: dest,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0
      });
      status = "exporté ✓";
      setTimeout(() => (status = status === "exporté ✓" ? "" : status), 2000);
    } catch (e) {
      status = `erreur : ${e}`;
    } finally {
      progress = null;
    }
  }

  // ---- import ---------------------------------------------------------------
  /** @param {Card} card */
  async function importCard(card) {
    // Prefer the explicitly-chosen import folder; fall back to the primary
    // catalogue root when none is set (preserves the pre-choice behavior).
    let archive = importDir ?? root;
    if (!archive) {
      archive = await invoke("pick_folder");
      if (!archive) return;
    }
    progress = { verb: "import", done: 0, total: card.raw_count, current: "" };
    lastImportedFolder = null;
    importingCard = card;
    ejectableCard = null;
    try {
      const stats = await invoke("import_card", { dcim: card.dcim, archive });
      await invoke("scan_root", { path: archive });
      await refreshDirs();
      if (stats.folders.length) {
        lastImportedFolder = stats.folders[stats.folders.length - 1];
        await refreshDirs();
        if (lastImportedFolder) await openDir(lastImportedFolder);
      }
      const summary = stats.cancelled
        ? `Import arrêté · ${stats.copied} importées`
        : `${stats.copied} importées · ${stats.skipped} ignorées · ${stats.failed} échecs`;
      appMessage = summary;
      invoke("notify_user", { title: "Reveal — import", body: summary }).catch(() => {});
      setTimeout(() => (appMessage = ""), 5000);
      // A real removable card (has a volume mount point) can now be ejected —
      // the ingest loop's last step, offered in the rail rather than forced.
      if (card.volume) ejectableCard = card;
    } finally {
      progress = null;
      importingCard = null;
      cards = await invoke("find_cards");
    }
  }

  function stopImport() {
    invoke("cancel_import").catch(() => {});
  }

  /**
   * Cull one just-imported day-folder down to `aiCullTarget` frames and
   * export them — Rust does prefilter -> vision ranking -> export in one
   * call (`ai_cull`), so this is just the invocation + error toast; progress
   * comes through the `cull-*` events listened for at boot.
   * @param {string} dir
   * @param {string[]} paths
   */
  async function triggerAiCull(dir, paths) {
    try {
      await invoke("ai_cull", { dir, paths });
    } catch (error) {
      appMessage = `Culling IA (${dir.split("/").pop()}) : ${error}`;
      setTimeout(() => (appMessage = ""), 6000);
    }
  }

  /**
   * The rail's manual "Culling IA" button: score the CURRENT folder's view
   * (same prefilter + vision ranking as the walk-away flow, via
   * `ai_cull_selection`) but instead of rating+exporting, add each pick to
   * the folder's quick collection — the same story-note mechanism the `q`
   * shortcut toggles. Only ADDS (never removes) — a photo already in the
   * collection is left alone rather than toggled out.
   */
  async function cullCurrentFolder() {
    const d = gridDir();
    if (!d || !view.length || progress) return;
    progress = { verb: "cull", done: 0, total: view.length, current: "" };
    appMessage = `Culling IA · ${view.length} photos…`;
    try {
      const result = await invoke("ai_cull_selection", { dir: d, paths: view.map((f) => f.path) });
      const currentStems = new Set(await invoke("story_stems", { dir: d }));
      let added = 0;
      for (const path of result.picked) {
        const s = stem(path.split("/").pop());
        if (currentStems.has(s)) continue;
        const updated = await invoke("story_toggle", { dir: d, path });
        storySet = new Set(updated);
        currentStems.add(s);
        added++;
      }
      await loadStory();
      refreshStoryDirs();
      appMessage = `Culling IA ✓ ${added} ajoutés à la collection rapide (${result.picked.length}/${result.considered} retenus)`;
    } catch (error) {
      appMessage = `Culling IA : ${error}`;
    } finally {
      progress = null;
      setTimeout(() => (appMessage = ""), 6000);
    }
  }

  /** @param {Card} card */
  async function ejectCard(card) {
    if (!card?.volume || ejecting) return;
    ejecting = true;
    try {
      await invoke("eject_card", { volume: card.volume });
      ejectableCard = null;
      appMessage = `${card.name} éjectée · tu peux retirer la carte`;
      setTimeout(() => (appMessage = ""), 5000);
    } catch (e) {
      appMessage = `Éjection échouée : ${typeof e === "string" ? e : String(e)}`;
      setTimeout(() => (appMessage = ""), 6000);
    } finally {
      ejecting = false;
      cards = await invoke("find_cards");
    }
  }

  // ---- export ---------------------------------------------------------------
  function saveExportPrefs() {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        "reveal.export",
        JSON.stringify({ edge: exportEdge, border: exportBorder, folder: exportFolder }),
      );
    }
  }

  // The Swift "DOSSIER" picker — chosen once, remembered; "" = the Desktop.
  async function chooseExportFolder() {
    const dest = await invoke("pick_folder");
    if (dest) {
      exportFolder = dest;
      saveExportPrefs();
      sendDevStateToPanel();
    }
  }

  /**
   * @param {string} label
   * @param {number} total
   */
  function createQueueJob(label, total) {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
    exportQueue = [
      ...exportQueue,
      {
        id,
        label,
        current: "Waiting",
        done: 0,
        total,
        phase: "Queued",
        status: "running",
        timestamp: new Date().toLocaleTimeString(),
      },
    ];
    activeExportJobId = id;
    queueOpen = true;
    return id;
  }

  /**
   * @param {string} id
   * @param {Partial<ExportJob>} patch
   */
  function updateQueueJob(id, patch) {
    exportQueue = exportQueue.map((job) => (job.id === id ? { ...job, ...patch } : job));
  }

  async function cancelExportQueue() {
    if (!activeExportJobId) return;
    updateQueueJob(activeExportJobId, { phase: "Cancelling after current photo…" });
    await invoke("cancel_exports");
  }

  async function exportGrid() {
    if (!view.length) return;
    const jobId = createQueueJob(`Export ${view.length} photos`, view.length);
    progress = { verb: "export", done: 0, total: view.length, current: "" };
    try {
      const completed = await invoke("export_photos", {
        paths: view.map((f) => f.path),
        destDir: exportFolder,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      const job = exportQueue.find((item) => item.id === jobId);
      if (job?.status !== "cancelled") {
        updateQueueJob(jobId, {
          current: "",
          done: completed,
          phase: completed === view.length ? "Complete" : "Stopped",
          status: completed === view.length ? "completed" : "cancelled",
        });
      }
    } catch (error) {
      updateQueueJob(jobId, {
        phase: String(error),
        status: "failed",
      });
    } finally {
      if (activeExportJobId === jobId) activeExportJobId = null;
      progress = null;
    }
  }

  // The Swift `r` — "Reveal": develop + export the current selection (or the
  // focused frame if nothing is multi-selected) with the active export params.
  async function exportSelection() {
    const targets = selectedFrames();
    if (!targets.length || progress) return;
    const jobId = createQueueJob(
      `Export ${targets.length} photo${targets.length > 1 ? "s" : ""}`,
      targets.length,
    );
    progress = { verb: "export", done: 0, total: targets.length, current: "" };
    try {
      const completed = await invoke("export_photos", {
        paths: targets.map((f) => f.path),
        destDir: exportFolder,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      updateQueueJob(jobId, {
        current: "",
        done: completed,
        phase: completed === targets.length ? "Complete" : "Stopped",
        status: completed === targets.length ? "completed" : "cancelled",
      });
    } catch (error) {
      updateQueueJob(jobId, { phase: String(error), status: "failed" });
    } finally {
      if (activeExportJobId === jobId) activeExportJobId = null;
      progress = null;
    }
  }

  /** @param {string} [destDir] override the configured export folder — used by the dev panel's quick-export-to-Desktop button */
  async function exportCurrent(destDir = exportFolder) {
    if (!photoPath || !recipe) return;
    const jobId = createQueueJob(`Export ${picked}`, 1);
    status = "Exporting…";
    try {
      await invoke("export_photo", {
        path: photoPath,
        recipe: { ...recipe },
        destDir,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      updateQueueJob(jobId, {
        current: picked ?? "",
        done: 1,
        phase: "Complete",
        status: "completed",
      });
      status = "Exported";
      setTimeout(() => (status = status === "Exported" ? "" : status), 2000);
    } catch (e) {
      updateQueueJob(jobId, { phase: String(e), status: "failed" });
      status = `Export failed: ${e}`;
    } finally {
      if (activeExportJobId === jobId) activeExportJobId = null;
    }
  }

  // Develop the current photo and drop the JPEG into the Obsidian vault's
  // attachment folder (Kernel/attachments here) — a filesystem export into the
  // vault, so the frame is ready to embed in a note. Distinct from Publish,
  // which pushes to the Garden.
  async function exportToVault() {
    if (!photoPath || !recipe) return;
    status = "Vers le vault…";
    try {
      const dest = await invoke("vault_attachment_dir");
      const out = await invoke("export_photo", {
        path: photoPath,
        recipe: { ...recipe },
        destDir: dest,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      status = `Dans le vault → ${out.split("/").slice(-2).join("/")}`;
      appMessage = status;
      setTimeout(() => (appMessage = ""), 4000);
      setTimeout(() => (status = ""), 3000);
    } catch (e) {
      status = `Échec vault : ${e}`;
    }
  }

  // ---- grille -------------------------------------------------------------
  async function pickFolder() {
    const path = await invoke("pick_folder");
    if (path) openFolder(path);
  }

  /** @param {string} path */
  async function openFolder(path) {
    folder = path;
    curDir = null;
    // Folder switch always starts with the full contact sheet.
    minRating = 0;
    filterStory = false;
    frames = await withPreviewVersions(await invoke("list_dir", { path }));
    sel = 0;
    selectOnly(0);
    if (typeof localStorage !== "undefined") {
      const savedMode = localStorage.getItem(`reveal.mode.${path}`);
      if (savedMode === "cull" || savedMode === "dev" || savedMode === "story") {
        await switchMode(savedMode);
      } else {
        await switchMode("cull");
      }
    } else {
      await switchMode("cull");
    }
    refreshStory();
  }

  /**
   * @param {string} path
   * @param {number} [version]
   */
  function thumbUrl(path, version = 0) {
    return `reveal://thumb?p=${encodeURIComponent(path)}&v=${version}`;
  }


  /** @param {number} n */
  async function rate(n) {
    const targets = selectedFrames();
    if (!targets.length) return;
    for (const frame of targets) frame.rating = n;
    frames = [...frames]; // raw array — reassign so the grid stars update
    await Promise.all(targets.map((frame) => invoke("set_rating", { path: frame.path, rating: n })));
  }

  /** @param {string} path */
  /** @param {string} path */
  function showCopiedMessage(path) {
    const filename = path.split("/").pop();
    appMessage = `Image copiée dans le presse-papier (${filename}) ✓`;
    setTimeout(() => {
      if (appMessage.startsWith("Image copiée")) appMessage = "";
    }, 2500);
  }

  // Every branch here writes straight to NSPasteboard from Rust rather than
  // going through the WebView's Clipboard API — reading the app's own
  // resources back through fetch()/<img>/canvas.toBlob() hit THREE separate
  // WebKit bugs in a row in this WebView (fetch() on our own blob: URL threw
  // "Load failed"; loading that into an <img> for canvas.toBlob() instead
  // threw SecurityError/tainted canvas; and fetch() on the reveal://thumb
  // custom protocol ALSO threw "Load failed", a pre-existing bug unrelated
  // to blob: URLs) — all reproduced 2026-08-02. Native NSPasteboard writes
  // sidestep the whole category. The sole exception is the Rapid engine's
  // canvas, which is painted from locally-decoded RGBA pixels
  // (putImageData), never a fetched/cross-origin image, so canvas.toBlob()
  // on it was never tainted.
  /** @param {string} path */
  async function copyImageToClipboard(path) {
    if (!path) return;
    try {
      if (currentMode === "dev" && useCanvas && canvasEl) {
        const canvas = canvasEl;
        const pngBlob = await new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
        if (pngBlob) {
          await navigator.clipboard.write([new ClipboardItem({ [pngBlob.type]: pngBlob })]);
          showCopiedMessage(path);
        }
        return;
      }

      if (currentMode === "dev" && photoPath === path && recipe) {
        await invoke("copy_developed_preview_to_clipboard", { path, recipe, maxPx: PREVIEW_PX });
      } else {
        await invoke("copy_photo_preview_to_clipboard", { path });
      }
      showCopiedMessage(path);
    } catch (err) {
      console.error("Could not copy image to clipboard:", err);
      appMessage = `Échec de la copie de l'image : ${err}`;
      setTimeout(() => (appMessage = ""), 3000);
    }
  }

  async function copySettings() {
    const source = view[sel];
    if (!source) return;
    /** @type {any} */
    let base = {};
    if (photoPath === source.path && recipe) {
      base = { ...recipe };
    } else {
      const [sidecar, defaults] = await Promise.all([
        invoke("load_sidecar", { path: source.path }),
        invoke("default_recipe"),
      ]);
      base = { ...defaults, ...(sidecar?.engine_settings ?? {}) };
    }
    // Never copy crop settings across photos
    delete base.crop_aspect;
    delete base.crop_angle;
    delete base.crop_x;
    delete base.crop_y;
    delete base.crop_w;
    delete base.crop_h;
    copiedRecipe = base;

    appMessage = `Settings copied from ${source.name}`;
    setTimeout(() => {
      if (appMessage === `Settings copied from ${source.name}`) appMessage = "";
    }, 2000);
  }

  // Render + persist one recipe onto a set of frames, updating the live loupe
  // if the open photo is among them. Shared by paste-settings and preset-apply.
  /**
   * @param {Recipe | null} recipeToApply
   * @param {Frame[]} targetFrames
   */
  async function applyRecipeToFrames(recipeToApply, targetFrames) {
    if (!recipeToApply || !targetFrames.length || progress) return;
    const snapshot = { ...recipeToApply };
    progress = { verb: "Applying settings", done: 0, total: targetFrames.length, current: "" };
    try {
      for (const frame of targetFrames) {
        progress = { ...progress, current: frame.name };

        // Load existing sidecar to preserve each destination photo's unique crop
        const existingSidecar = await invoke("load_sidecar", { path: frame.path }).catch(() => null);
        const existingSettings = existingSidecar?.engine_settings ?? {};

        const frameRecipe = {
          ...snapshot,
          crop_aspect: existingSettings.crop_aspect ?? "original",
          crop_angle: existingSettings.crop_angle ?? 0,
          crop_x: existingSettings.crop_x ?? 0,
          crop_y: existingSettings.crop_y ?? 0,
          crop_w: existingSettings.crop_w ?? 1,
          crop_h: existingSettings.crop_h ?? 1,
        };

        const bytes = await invoke("develop_preview", {
          path: frame.path,
          recipe: frameRecipe,
          maxPx: PREVIEW_PX,
        });
        await invoke("save_recipe", { path: frame.path, recipe: frameRecipe });
        frame.previewVersion = Date.now();
        progress = { ...progress, done: progress.done + 1 };

        if (frame.path === photoPath) {
          recipe = { ...frameRecipe };
          developEngine = frameRecipe.engine === "rapid" ? "rapid" : "spektra";
          if (developEngine === "rapid") {
            scheduleRender(PREVIEW_PX);
          } else {
            useCanvas = false;
            if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
            imgUrl = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
          }
          sendDevStateToPanel();
        }
        frames = [...frames]; // reassign per photo so grid thumbs update live as each frame develops
      }
      appMessage = `Settings applied to ${targetFrames.length} photo${targetFrames.length === 1 ? "" : "s"}`;
      setTimeout(() => {
        if (appMessage.startsWith("Settings applied")) appMessage = "";
      }, 2500);
    } catch (e) {
      appMessage = `Could not apply settings: ${e}`;
    } finally {
      progress = null;
    }
  }

  async function pasteSettings() {
    if (!copiedRecipe) return;
    await applyRecipeToFrames(copiedRecipe, selectedFrames());
  }

  /** @param {any} res */
  function applyWorkflowResult(res) {
    if (!res || res.action === "NONE") return false;
    if (res.action === "SWITCH_MODE") {
      switchMode(res.to, { openDevPanel: res.openDevPanel ?? true });
      if (res.spaceLook !== undefined) spaceLook = res.spaceLook;
      return true;
    }
    if (res.action === "OPEN_DEV_PANEL") {
      spaceLook = res.spaceLook ?? false;
      layouts.dev.devPanel = true;
      saveLayouts();
      if (currentMode !== "dev") {
        switchMode("dev", { openDevPanel: true });
      }
      return true;
    }
    if (res.action === "TOGGLE_SIDEBAR") {
      toggleSidebar();
      return true;
    }
    if (res.action === "CLOSE_OVERLAY") {
      if (photoMenu) closePhotoMenu();
      else if (shortcutsOpen) shortcutsOpen = false;
      else if (settingsOpen) settingsOpen = false;
      return true;
    }
    if (res.action === "EXIT_FULLSCREEN") {
      exitFullscreen();
      return true;
    }
    return false;
  }

  /** @param {KeyboardEvent} e */
  function onKey(e) {
    if (["INPUT", "SELECT", "TEXTAREA"].includes(/** @type {HTMLElement} */ (e.target).tagName)) return;

    // ⌘A / Ctrl-A — select all
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
      selectedPaths = new Set(view.map(f => f.path));
      e.preventDefault();
      return;
    }

    // ⌘C / Ctrl-C — copy photo image to OS clipboard
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "c") {
      const targetPath = currentMode === "dev" ? photoPath : (view[sel]?.path || photoPath);
      if (targetPath) {
        copyImageToClipboard(targetPath);
        e.preventDefault();
        return;
      }
    }

    // ⌘D / Ctrl-D — deselect all (clear the focused frame + the ⌘-click set).
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "d") {
      selectedPaths = new Set();
      e.preventDefault();
      return;
    }

    if (e.key.toLowerCase() === "f") {
      toggleFullscreen();
      e.preventDefault();
      return;
    }
    if (fullscreen && (e.key === "Escape" || e.key === " " || e.key.toLowerCase() === "g")) {
      exitFullscreen();
      e.preventDefault();
      return;
    }

    // Global layout toggles
    if (e.key === "o") {
      toggleFocusMode();
      e.preventDefault();
      return;
    }
    if (e.key === "l") {
      toggleAppearance();
      e.preventDefault();
      return;
    }
    if (e.key === "b") {
      toggleSidebar();
      e.preventDefault();
      return;
    }
    if (e.key === "m") {
      toggleLayout();
      e.preventDefault();
      return;
    }
    if (e.key === "q") {
      toggleStory(); // Toggle selected photo in story note
      e.preventDefault();
      return;
    }
    // Grid zoom (Swift `+`/`−`): the =/+ and -/_ keys. No shift steps the
    // column count (fewer = bigger photos, more = smaller); shift on the same
    // key steps the grid margin instead — the paired axis under one gesture.
    // In JS the shifted variants arrive as "+"/"_", so the base char already
    // tells the two axes apart.
    if (!e.metaKey && !e.ctrlKey) {
      if (e.key === "=") {
        cols = Math.max(1, cols - 1);
        saveGridPrefs();
        e.preventDefault();
        return;
      }
      if (e.key === "+") {
        marginScale = Math.min(6, Math.round((marginScale + 0.25) * 100) / 100);
        saveGridPrefs();
        e.preventDefault();
        return;
      }
      if (e.key === "-") {
        cols = Math.min(12, cols + 1);
        saveGridPrefs();
        e.preventDefault();
        return;
      }
      if (e.key === "_") {
        marginScale = Math.max(0.25, Math.round((marginScale - 0.25) * 100) / 100);
        saveGridPrefs();
        e.preventDefault();
        return;
      }
    }
    if (!e.metaKey && !e.ctrlKey && e.key.toLowerCase() === "c") {
      copySettings();
      e.preventDefault();
      return;
    }
    if (!e.metaKey && !e.ctrlKey && e.key.toLowerCase() === "v") {
      pasteSettings();
      e.preventDefault();
      return;
    }
    if (e.key.toLowerCase() === "d" && e.shiftKey) {
      toggleDevPanel();
      e.preventDefault();
      return;
    }
    if (e.key.toLowerCase() === "z" && !e.metaKey && !e.ctrlKey && currentMode === "dev") {
      cycleZoom(e.shiftKey);
      e.preventDefault();
      return;
    }
    if (e.key === "?" || e.key === "h") {
      shortcutsOpen = !shortcutsOpen;
      e.preventDefault();
      return;
    }
    if (e.key === "r" && !e.metaKey && !e.ctrlKey) {
      // Swift `r` — "Reveal": develop + export the selection. The render queue
      // still surfaces automatically (createQueueJob opens it) and stays
      // reachable from the native menu.
      exportSelection();
      e.preventDefault();
      return;
    }

    // Mode switcher shortcuts handled via AppController
    const controller = new AppController({
      currentMode,
      spaceLook,
      devPanel: layouts.dev.devPanel
    });

    if (e.key === "g") {
      applyWorkflowResult(controller.handleG());
      e.preventDefault();
      return;
    }
    if (e.key === "d" && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
      applyWorkflowResult(controller.handleD());
      e.preventDefault();
      return;
    }
    if (e.key === "s") {
      applyWorkflowResult(controller.handleS());
      e.preventDefault();
      return;
    }
    if (e.key === "Escape") {
      const hasOverlay = !!(photoMenu || shortcutsOpen || settingsOpen);
      applyWorkflowResult(controller.handleEscape({ hasOverlay, fullscreen }));
      e.preventDefault();
      return;
    }

    // In Develop mode (single photo view), ArrowUp / ArrowDown modifies the last edited setting!
    if (currentMode === "dev" && (e.key === "ArrowUp" || e.key === "ArrowDown") && !e.metaKey && !e.ctrlKey) {
      if (recipe && lastEditedKey) {
        const config = SETTING_STEPS[lastEditedKey] || { step: 0.05, shiftStep: 0.25 };
        const step = e.shiftKey ? config.shiftStep : config.step;
        const currentVal = Number(/** @type {any} */ (recipe)[lastEditedKey] ?? 0);
        const delta = e.key === "ArrowUp" ? step : -step;
        const newVal = Math.round((currentVal + delta) * 1000) / 1000;
        
        recipe = { ...recipe, [lastEditedKey]: newVal };
        if (!developEngine || developEngine === "none") developEngine = "spektra";
        recipe.engine = developEngine;
        
        edited(true);
        sendDevStateToPanel();
        e.preventDefault();
        return;
      }
    }

    const c = (fullscreen || currentMode === "dev") ? 1 : cols;
    const isNav = ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key);
    
    if (isNav) {
      let nextSel = sel;
      if (e.key === "ArrowRight") nextSel = Math.min(sel + 1, view.length - 1);
      else if (e.key === "ArrowLeft") nextSel = Math.max(sel - 1, 0);
      else if (e.key === "ArrowDown") nextSel = Math.min(sel + c, view.length - 1);
      else if (e.key === "ArrowUp") nextSel = Math.max(sel - c, 0);
      
      sel = nextSel;

      if (e.shiftKey) {
        const from = Math.min(selectionAnchor, sel);
        const to = Math.max(selectionAnchor, sel);
        const next = new Set();
        for (let i = from; i <= to; i += 1) next.add(view[i].path);
        selectedPaths = next;
      } else if (e.metaKey || e.ctrlKey) {
        // macOS/Windows pattern: Cmd/Ctrl + Arrow just moves the cursor (sel) without changing selection.
      } else {
        selectOnly(sel);
      }
      
      e.preventDefault();
      if (fullscreen) {
        prepareFullscreenFrame(view[sel].path);
      } else if (currentMode === "dev" && view[sel]) {
        openPhoto(view[sel].path);
      }
      document.querySelector(`[data-idx="${sel}"]`)?.scrollIntoView({ block: "nearest" });
      return;
    }

    if (e.key === " " && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
      if (currentMode === "cull") {
        if (!selectedPaths.size && !view[sel]) return;
        photoPath = view[sel]?.path;
        if (!photoPath) return;
      }
      applyWorkflowResult(controller.handleSpace());
      e.preventDefault();
      return;
    }
    // Toggle selection with space when holding Cmd/Ctrl (like macOS Finder)
    else if (e.key === " " && (e.metaKey || e.ctrlKey)) {
      if (view[sel]) {
        const next = new Set(selectedPaths);
        if (next.has(view[sel].path)) next.delete(view[sel].path);
        else next.add(view[sel].path);
        selectedPaths = next;
        selectionAnchor = sel;
      }
      e.preventDefault();
      return;
    }
    else if (e.key >= "0" && e.key <= "5") rate(Number(e.key));
    else return;

    e.preventDefault();
  }

  // ---- develop ------------------------------------------------------------
  /** @param {string} path */
  /**
   * @param {string} path
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  async function openPhoto(path, { openDevPanel = true } = {}) {
    if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
    photoPath = path;
    picked = path.split("/").pop() ?? null;
    imgUrl = thumbUrl(path);
    useCanvas = false; // start on the <img> thumb; pump() flips this back on
    // only if the photo develops with a canvas (Rapid) engine.
    imgFailed = false;
    status = "";
    try {
      const [sidecar, defaults] = await Promise.all([
        invoke("load_sidecar", { path }),
        invoke("default_recipe"),
      ]);
      if (path !== photoPath) return;
      caption = sidecar?.description ?? "";
      developEngine = sidecar?.engine
        ? sidecar.engine
        : (sidecar?.engine_settings ? "spektra" : null);
      recipe = {
        ...defaults,
        ...(sidecar?.engine_settings ?? {}),
      };
      if (developEngine) {
        scheduleRender(PREVIEW_PX);
      } else {
        // Moteur « Aucun » : on affiche l'aperçu as-shot (le JPEG caméra / le
        // .reveal.jpg que la grille montre déjà via reveal://thumb), pas un
        // développement couleur du RAW. Toucher un réglage réengage le moteur
        // via edited(). Ça évite aussi de polluer le cache .reveal.jpg.
        status = "";
      }
      if (currentMode !== "dev") {
        // Double-click: single-photo view only, panel stays closed until "D"
        // is pressed — same "quick look" semantics Space already uses
        // (reproduced 2026-08-04, matches the existing spaceLook mechanism
        // rather than inventing a second one).
        if (!openDevPanel) spaceLook = true;
        await switchMode("dev", { openDevPanel });
      }
    } catch (error) {
      status = "Could not load photo";
      appMessage = `Could not open photo: ${error}`;
      setTimeout(() => {
        if (appMessage.startsWith("Could not open photo:")) appMessage = "";
      }, 5000);
    }
  }

  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let captionTimer = undefined;
  function captionEdited() {
    clearTimeout(captionTimer);
    const path = photoPath;
    captionTimer = setTimeout(() => {
      if (path) invoke("save_caption", { path, description: caption });
    }, 400);
  }

  let inflight = $state(false);
  /** @type {number | null} */ let pendingPx = $state(null);

  /** @param {number} px */
  function scheduleRender(px) {
    pendingPx = px; // latest wins
    pump();
  }

  async function pump() {
    if (inflight || pendingPx === null || !photoPath || !recipe) return;
    inflight = true;
    const px = pendingPx;
    pendingPx = null;
    const path = photoPath;
    const snap = { ...recipe };
    const t0 = performance.now();
    try {
      if (snap.engine === "rapid") {
        const res = await invoke("develop_preview_rgba", { path, recipe: snap, maxPx: px });
        if (path === photoPath) {
          renderMs = Math.round(performance.now() - t0);
          useCanvas = true;
          // Reactive aspect for the canvas path — canvasEl.width is a DOM
          // mutation the view can't track, so the mat sizing (breathing room)
          // would never apply and the margins broke. Feed it explicitly.
          renderAspect = res.height ? res.width / res.height : null;
          await tick();
          if (canvasEl) {
            canvasEl.width = res.width;
            canvasEl.height = res.height;
            const ctx = canvasEl.getContext("2d");
            if (ctx) {
              const imgData = new ImageData(new Uint8ClampedArray(res.rgba), res.width, res.height);
              ctx.putImageData(imgData, 0, 0);
            }
          }
          imgFailed = false;
          status = "";
        }
      } else {
        useCanvas = false;
        const bytes = await invoke("develop_preview", { path, recipe: snap, maxPx: px });
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
        const img = new Image();
        img.src = url;
        try { await img.decode(); } catch (e) {}

        if (path === photoPath) {
          renderMs = Math.round(performance.now() - t0);
          if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
          imgUrl = url;
          imgFailed = false;
          const frame = frames.find((item) => item.path === path);
          if (frame) {
            frame.previewVersion = Date.now();
            frames = [...frames]; // raw array — reassign so the grid thumb refreshes
          }
          status = "";
        } else {
          URL.revokeObjectURL(url);
        }
      }
    } catch (e) {
      if (path === photoPath) status = `Error: ${e}`;
    } finally {
      inflight = false;
      if (pendingPx !== null) pump();
    }
  }

  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let saveTimer = undefined;
  function edited(live = false) {
    if (!developEngine || developEngine === "none") {
      developEngine = "spektra"; // touching a control defaults to the analog engine
    }
    if (recipe) {
      recipe.engine = developEngine; // developEngine holds the Rust engine id
    }
    scheduleRender(live ? DRAG_PX : PREVIEW_PX);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      if (photoPath && recipe) invoke("save_recipe", { path: photoPath, recipe: { ...recipe } });
    }, 300);
  }

  async function clearDevelopment() {
    if (!photoPath) return;
    const path = photoPath;
    await invoke("clear_recipe", { path });
    if (path !== photoPath) return;
    developEngine = null;
    const frame = frames.find((item) => item.path === path);
    if (frame) frame.previewVersion = Date.now();
    frames = [...frames]; // raw array — reassign so the grid thumb refreshes
    if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
    imgUrl = thumbUrl(path, frame?.previewVersion ?? Date.now());
    useCanvas = false; // the thumb is a plain <img>; leaving useCanvas true (from
    // a prior Rapid render) would show the now-blank <canvas> instead.
    status = "";
  }


  /** @param {number | string} v */
  const fmt = (v) => Number(v).toFixed(2).replace(/\.?0+$/, "") || "0";
  /** @param {number} n */
  const stars = (n) => "★".repeat(n);

  // The star menu sets an absolute threshold (Tout / ≥ N★), like Swift's menu.
  /** @param {number} n */
  function setMinRating(n) {
    minRating = n;
    if (curDir) openDir(curDir);
  }

  function closeMenus() {
    starMenuOpen = sortMenuOpen = layoutMenuOpen = false;
  }

  /** @type {[string, number][]} */
  const aspects = [
    ["1:1", 1],
    ["4:5", 0.8],
    ["5:4", 1.25],
    ["3:2", 1.5],
    ["2:3", 0.667],
    ["16:9", 1.778],
  ];

  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let catalogTimer = undefined;
  /** @param {string} v */
  function catalogEdited(v) {
    catalogContent = v;
    clearTimeout(catalogTimer);
    catalogTimer = setTimeout(saveCatalogNote, 600);
  }

  let sidebarPeek = $state(false);
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let sidebarPeekTimer = undefined;
  function openSidebarPeek() {
    if (!root || layouts[currentMode].sidebar) return;
    clearTimeout(sidebarPeekTimer);
    sidebarPeek = true;
  }
  function scheduleSidebarPeekClose() {
    clearTimeout(sidebarPeekTimer);
    sidebarPeekTimer = setTimeout(() => (sidebarPeek = false), 380);
  }
  function closeSidebarPeek() {
    clearTimeout(sidebarPeekTimer);
    sidebarPeek = false;
  }

  const sidebarVisible = $derived(!!root && layouts[currentMode].sidebar);
</script>

<svelte:window onkeydown={onKey} />

{#if isTauri}
  <div
    class="window-controls"
    class:fullscreen
    class:dev={currentMode === "dev"}
    class:away={(fullscreen || currentMode === "dev") && !pointerInside}
    aria-label="Window controls"
  >
    <button class="window-close" onclick={closeMainWindow} aria-label="Close window"></button>
    <button class="window-minimize" onclick={minimizeMainWindow} aria-label="Minimize window"></button>
    <button class="window-zoom" onclick={zoomMainWindow} aria-label="Zoom window"></button>
  </div>
{/if}

{#if currentMode === "cull" || currentMode === "story"}
  <div class="cull" role="presentation">
    <div class="body">
      {#if sidebarVisible}
        <Sidebar
          {root}
          {roots}
          {dirs}
          {curDir}
          {scanning}
          {indexProgress}
          mode={currentMode}
          {storyDirs}
          focusOn={layouts[currentMode].focus}
          {catalogContent}
          {importDir}
          onOpenDir={openDir}
          onOpenLibrary={() => openDir(root)}
          onRescan={rescan}
          onRescanDir={rescanDir}
          onRevealDir={revealDir}
          onAddLocation={indexRoot}
          onSetImportDir={setImportDir}
          onOpenNote={() => (catalogOpen = true)}
          onMode={(/** @type {"cull" | "dev" | "story"} */ m) => switchMode(m)}
          onToggleSidebar={toggleSidebar}
          onToggleFocus={toggleFocusMode}
          onToggleAppearance={toggleAppearance}
          onShowShortcuts={() => (shortcutsOpen = true)}
          onShowSettings={openSettings}
          onCatalogChange={catalogEdited}
          garden={gardenAccount}
          onGardenSignIn={gardenSignIn}
          onGardenSignOut={gardenSignOut}
          onOpenUrl={openUrl}
          onMovePhotos={movePhotos}
          selectedCount={selectedPaths.size || (view[sel] ? 1 : 0)}
          onMoveSelectedPhotos={moveSelectedPhotosToDir}
          onRenameDir={renameDir}
          onCreateFolder={createFolder}
          onMoveDir={moveDir}
          {pinnedStories}
          {recentStories}
          onDevelopStory={exportLocalStory}
          onPublishStory={publishStory}
          onExportLocalStory={exportLocalStory}
          onSetPinned={setStoryPinned}
          onReorderPinned={reorderPinned}
          publishing={!!progress}
          publishStatus={status}
        />
      {/if}
      {#if sidebarPeek && !sidebarVisible}
        <div
          class="sidebar-peek"
          role="region"
          aria-label="Folder browser preview"
          onmouseenter={openSidebarPeek}
          onmouseleave={scheduleSidebarPeekClose}
        >
          <Sidebar
            {root}
            {roots}
            {dirs}
            {curDir}
            {scanning}
            {indexProgress}
            mode={currentMode}
            {storyDirs}
            focusOn={layouts[currentMode].focus}
            {catalogContent}
            {importDir}
            floating
            onOpenDir={(/** @type {string} */ path) => {
              openDir(path);
              closeSidebarPeek();
            }}
            onOpenLibrary={() => {
              openDir(root);
              closeSidebarPeek();
            }}
            onRescan={rescan}
            onRescanDir={rescanDir}
            onRevealDir={revealDir}
            onAddLocation={indexRoot}
            onSetImportDir={setImportDir}
            onOpenNote={() => (catalogOpen = true)}
            onMode={(/** @type {"cull" | "dev" | "story"} */ m) => switchMode(m)}
            onToggleSidebar={toggleSidebar}
            onToggleFocus={toggleFocusMode}
            onToggleAppearance={toggleAppearance}
            onShowShortcuts={() => (shortcutsOpen = true)}
            onShowSettings={openSettings}
            onCatalogChange={catalogEdited}
            garden={gardenAccount}
            onGardenSignIn={gardenSignIn}
            onGardenSignOut={gardenSignOut}
            onOpenUrl={openUrl}
            onMovePhotos={movePhotos}
            selectedCount={selectedPaths.size || (view[sel] ? 1 : 0)}
            onMoveSelectedPhotos={moveSelectedPhotosToDir}
            onRenameDir={renameDir}
            onCreateFolder={createFolder}
            onMoveDir={moveDir}
            {pinnedStories}
            {recentStories}
            onDevelopStory={exportLocalStory}
            onPublishStory={publishStory}
            onExportLocalStory={exportLocalStory}
            onSetPinned={setStoryPinned}
            onReorderPinned={reorderPinned}
            publishing={!!progress}
            publishStatus={status}
            {gardenUrl}
          />
        </div>
      {/if}
      <div class="content" onmousedown={startWindowDrag}>
        <!-- The top rail — canvas-toned, one toolbar line across the window;
             the brand cluster only rides here when the sidebar isn't inline. -->
        <header class="rail" class:solo={!sidebarVisible} data-tauri-drag-region>
          {#if !sidebarVisible}
            <div class="brand-cluster">
              <button
                class="chrome-btn"
                onclick={toggleSidebar}
                onmouseenter={openSidebarPeek}
                onmouseleave={scheduleSidebarPeekClose}
                title="Folder panel (B)"
              >
                <Icon name="sidebar-simple" size="12px" />
              </button>
              <button
                class="chrome-btn"
                class:on={layouts[currentMode].focus}
                onclick={toggleFocusMode}
                title="Mode Focus — estompe l'arrière-plan (o)"
              >
                <span class="focus-glyph" class:on={layouts[currentMode].focus}></span>
              </button>
              <button class="chrome-btn" onclick={toggleAppearance} title="Basculer le mode clair / sombre du système (l)">
                <Icon name="circle-half" size="12px" />
              </button>
              <button class="wordmark" onclick={() => (shortcutsOpen = true)} title="Raccourcis clavier">
                {curDir && curDir !== root ? (dirLabel(curDir) ?? "").toUpperCase() : "REVEAL"}
              </button>
            </div>
          {/if}

          <!-- Star filter — icon-only; the threshold reads in the menu. -->
          <div class="rail-menu">
            <button
              class="rail-btn"
              class:on={minRating > 0 || filterStory}
              onclick={() => {
                const open = starMenuOpen;
                closeMenus();
                starMenuOpen = !open;
              }}
              title={minRating > 0 ? `Filtre : ≥ ${minRating}★` : "Filtrer par note"}
            >
              <Icon name="star" size="12px" />
            </button>
            {#if starMenuOpen}
              <div class="popover">
                <button
                  class="pop-row"
                  onclick={() => {
                    filterStory = !filterStory;
                  }}
                >
                  <span>Collection Rapide</span>
                  {#if filterStory}<Icon name="check" size="10px" />{/if}
                </button>
                <div class="pop-divider"></div>
                {#each [0, 1, 2, 3, 4, 5] as n}
                  <button class="pop-row" onclick={() => setMinRating(n)}>
                    <span>{n === 0 ? "Tout" : `≥ ${n}★`}</span>
                    {#if minRating === n}<Icon name="check" size="10px" />{/if}
                  </button>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Sort -->
          <div class="rail-menu">
            <button
              class="rail-btn"
              onclick={() => {
                const open = sortMenuOpen;
                closeMenus();
                sortMenuOpen = !open;
              }}
              title="Trier"
            >
              <Icon name="arrows-down-up" size="12px" />
            </button>
            {#if sortMenuOpen}
              <div class="popover">
                <button
                  class="pop-row"
                  onclick={() => {
                    sortDesc = false;
                    saveGridPrefs();
                  }}
                >
                  <span>Plus ancien d'abord</span>
                  {#if !sortDesc}<Icon name="check" size="10px" />{/if}
                </button>
                <button
                  class="pop-row"
                  onclick={() => {
                    sortDesc = true;
                    saveGridPrefs();
                  }}
                >
                  <span>Plus récent d'abord</span>
                  {#if sortDesc}<Icon name="check" size="10px" />{/if}
                </button>
              </div>
            {/if}
          </div>

          <span class="frame-count">
            {#if view.length !== frames.length}
              {view.length}/{frames.length} FRAMES
            {:else}
              {view.length} FRAMES
            {/if}
          </span>

          <span class="rail-spacer"></span>

          {#if !root}
            <button class="rail-action" onclick={indexRoot} disabled={!isTauri || scanning}>
              {scanning ? "indexation…" : "Indexer une bibliothèque"}
            </button>
          {/if}
          <!-- Idle cards: one import button each. The card mid-import shows
               the live chip below instead, so hide its button. -->
          {#each cards as card (card.dcim)}
            {#if !importingCard || importingCard.dcim !== card.dcim}
              <button class="import rail-action" onclick={() => importCard(card)} disabled={!!progress}>
                Importer {card.name} ({card.raw_count})
              </button>
            {/if}
          {/each}

          <!-- A finished card import awaiting ejection — pull the card safely. -->
          {#if ejectableCard}
            <button
              class="rail-action"
              onclick={() => { if (ejectableCard) ejectCard(ejectableCard); }}
              disabled={ejecting}
            >
              {ejecting ? "Éjection…" : `Éjecter ${ejectableCard.name}`}
            </button>
          {/if}

          {#if appMessage}
            <span class="progress">{appMessage}</span>
          {/if}
          {#if liveUrl}
            <span class="progress">{liveUrl}</span>
          {/if}

          <!-- The live import chip — same DIN/mono treatment as export, plus
               a stop control; the copy finishes its current file then halts. -->
          {#if progress && progress.verb === "import"}
            <span class="export-chip">
              {#if progress.path}
                <!-- Live preview of the frame being copied (embedded RAW JPEG),
                     keyed on the path so each new file swaps the image. -->
                {#key progress.path}
                  <img class="chip-thumb" src={thumbUrl(progress.path)} alt="" />
                {/key}
              {/if}
              <span class="chip-label">Importe</span>
              <span class="chip-count">{progress.done}/{progress.total}</span>
              <span class="chip-bar">
                <span
                  class="chip-fill"
                  style="width: {progress.total ? (progress.done / progress.total) * 100 : 0}%"
                ></span>
              </span>
              <button class="chip-stop" onclick={stopImport} title="Arrêter l'import">
                <Icon name="x" size="9px" />
              </button>
            </span>
          {/if}

          <!-- The live export chip — DIN label, mono count, a thin accent
               bar filling as frames finish; gone when the batch ends. -->
          {#if progress && progress.verb === "export"}
            <span class="export-chip">
              <span class="chip-label">Développe</span>
              <span class="chip-count">{progress.done}/{progress.total}</span>
              <span class="chip-bar">
                <span
                  class="chip-fill"
                  style="width: {progress.total ? (progress.done / progress.total) * 100 : 0}%"
                ></span>
              </span>
            </span>
          {/if}

          {#if (curDir || folder) && view.length}
            <button class="rail-action" onclick={exportGrid} disabled={!!progress}>Exporter</button>
            <button class="rail-action" onclick={cullCurrentFolder} disabled={!!progress}>Culling IA</button>
          {/if}
          {#if storySet.size}
            <button class="import rail-action" onclick={publishStory} disabled={!!progress}>
              Publier ({storySet.size})
            </button>
          {/if}

          <!-- The grid's whole geometry behind ONE icon. -->
          <div class="rail-menu">
            <button
              class="rail-btn"
              onclick={() => {
                const open = layoutMenuOpen;
                closeMenus();
                layoutMenuOpen = !open;
              }}
              title="Disposition de la grille"
            >
              <Icon name={layout === "masonry" ? "rows" : "grid-four"} size="12px" />
            </button>
            {#if layoutMenuOpen}
              <div class="popover layout-pop">
                <span class="pop-label">Colonnes</span>
                <div class="pop-grid">
                  {#each [1, 2, 3, 4, 5, 6, 8, 10, 12] as n}
                    <button
                      class="pop-chip"
                      class:on={cols === n}
                      onclick={() => {
                        cols = n;
                        saveGridPrefs();
                      }}
                    >{n}</button>
                  {/each}
                </div>
                <div class="pop-divider"></div>
                <span class="pop-label">Format</span>
                <button class="pop-row" onclick={toggleLayout}>
                  <span>Mosaïque</span>
                  {#if layout === "masonry"}<Icon name="check" size="10px" />{/if}
                </button>
                {#if layout !== "masonry"}
                  <div class="pop-grid three">
                    {#each aspects as [label, a]}
                      <button
                        class="pop-chip"
                        class:on={Math.abs(cellAspect - a) < 0.001}
                        onclick={() => {
                          cellAspect = a;
                          saveGridPrefs();
                        }}
                      >{label}</button>
                    {/each}
                  </div>
                  <button
                    class="pop-row"
                    onclick={() => {
                      fillCells = !fillCells;
                      saveGridPrefs();
                    }}
                  >
                    <span>{fillCells ? "Remplir les cases" : "Garder les proportions"}</span>
                    {#if !fillCells}<Icon name="check" size="10px" />{/if}
                  </button>
                {/if}
                <div class="pop-divider"></div>
                <span class="pop-label">Marge</span>
                <input
                  class="pop-slider"
                  type="range"
                  min="0.25"
                  max="6"
                  step="0.25"
                  bind:value={marginScale}
                  style="--f: {pct(marginScale, 0.25, 6)}"
                  onchange={saveGridPrefs}
                />
              </div>
            {/if}
          </div>
        </header>

        {#if starMenuOpen || sortMenuOpen || layoutMenuOpen}
          <div
            class="pop-backdrop"
            onclick={closeMenus}
            role="presentation"
          ></div>
        {/if}

      {#if currentMode === "cull"}
        <CullView
          {view}
          {loading}
          {sel}
          {selectedPaths}
          {storySet}
          {layout}
          {cols}
          {marginScale}
          {cellAspect}
          {fillCells}
          {progress}
          bind:currentScrollTop={currentScrollTop}
          {curDir}
          {minRating}
          {isTauri}
          {debug}
          {thumbUrl}
          {selectGridItem}
          {openPhoto}
          {openPhotoMenu}
          {toggleStoryWithPath}
          {onPhotoDragStart}
          {closePhotoMenu}
          hasRoot={!!root}
          {scanning}
          onAddLibraryFolder={indexRoot}
        />
      {:else if currentMode === "story"}
        <StoryView
          {view}
          {storySet}
          {storyContent}
          {progress}
          liveUrl={liveUrl ?? undefined}
          {thumbUrl}
          {saveStoryContent}
        />
      {/if}
      </div>
    </div>
    <ContextMenu
      {photoMenu}
      {selectedPaths}
      {installedEditors}
      {copiedRecipe}
      {storySet}
      {stem}
      onClose={closePhotoMenu}
      onOpenPhoto={(/** @type {string} */ p) => openPhoto(p)}
      onOpenPreview={(/** @type {string} */ p) => openPhotoPreview(p)}
      onRevealInFinder={(/** @type {string} */ p) => revealPhotoInFinder(p)}
      onOpenInEditor={(/** @type {string} */ p, /** @type {string} */ app) => openPhotoInEditor(p, app)}
      onCopyImage={(/** @type {string} */ p) => copyImageToClipboard(p)}
      onCopySettings={copySettings}
      onPasteSettings={pasteSettings}
      onRate={(/** @type {number} */ n) => rate(n)}
      onToggleStory={(/** @type {string} */ p) => toggleStoryWithPath(p)}
      onExportSelection={exportSelection}
      onDevelopToVault={(/** @type {string} */ p) => developFromMenu(p, true)}
    />
    {#if progress}
      <div class="import-hud">
        <strong>{progress.verb}</strong>
        <span>{progress.done}/{progress.total}</span>
        {#if progress.current}<em>{progress.current}</em>{/if}
      </div>
    {/if}
    {#if false && layouts[currentMode].focus}
      <div class="focus-overlay" aria-hidden="true"></div>
    {/if}

    {#if catalogOpen}
      <div class="overlay-modal" onclick={() => (catalogOpen = false)} role="presentation">
        <div class="modal-card" onclick={(e) => e.stopPropagation()} role="presentation">
          <div class="modal-header">
            <h3>CATALOGUE NOTE (REVEAL.MD)</h3>
            <button class="close-btn" onclick={() => (catalogOpen = false)}>✕</button>
          </div>
          <div class="modal-body">
            <textarea
              bind:value={catalogContent}
              placeholder="Write global notes for this library…"
            ></textarea>
          </div>
          <div class="modal-footer">
            <button onclick={async () => { await saveCatalogNote(); catalogOpen = false; }}>Save</button>
            <button class="secondary" onclick={() => (catalogOpen = false)}>Close</button>
          </div>
        </div>
      </div>
    {/if}

    {#if queueOpen}
      <RenderQueueModal
        {exportQueue}
        {activeExportJobId}
        onClose={() => (queueOpen = false)}
        onCancelQueue={cancelExportQueue}
      />
    {/if}
  </div>
{:else}
  <div
    class="app"
    style="grid-template-columns: {(layouts.dev.devPanel && !isTauri) ? '1fr 19.5rem' : '1fr'};"
    onmousedown={startWindowDrag}
  >
    <DevelopView
      {picked}
      imgUrl={imgUrl ?? undefined}
      {useCanvas}
      {showClipping}
      {recipe}
      {renderAspect}
      bind:canvasEl={canvasEl}
      bind:imgFailed={imgFailed}
      {status}
      {inflight}
      {pendingPx}
      {zoomMode}
      {panning}
      {developPhotoPercent}
      {trackDevelopViewport}
      {onPhotoPointerDown}
      {onPhotoPointerMove}
      {onPhotoPointerUp}
    />
    {#if recipe && layouts.dev.devPanel && !isTauri}
      <aside>
        <button class="open" onclick={() => switchMode("cull")}>← Grille (g)</button>
        {#if picked}
          <p class="file">
            {picked}{renderMs ? ` · ${renderMs} ms` : ""}{status ? ` · ${status}` : ""}
          </p>
          {#if installedEditors.length}
            <div class="editor-select-container">
              <select class="editor-select" onchange={(e) => openInEditor(/** @type {HTMLSelectElement} */ (e.currentTarget).value)} value="">
                <option value="" disabled selected>Ouvrir dans...</option>
                {#each installedEditors as [name, path]}
                  <option value={path}>{name}</option>
                {/each}
              </select>
            </div>
          {/if}
        {/if}
        <!-- Section: Base -->
        <section class="collapsible">
          <button class="section-toggle" onclick={() => baseOpen = !baseOpen}>
            <span>BASE</span>
            <span class="chevron">{baseOpen ? "▼" : "▶"}</span>
          </button>
          {#if baseOpen}
            <div class="section-content">
              <label class="row check">
                <span>AUTO-EXPO</span>
                <input type="checkbox" bind:checked={recipe.auto_exposure} onchange={() => edited()} />
              </label>
              <label class="row">
                <span>EXPOSITION</span>
                <input type="range" min="-3" max="3" step="0.1" bind:value={recipe.exposure_ev} style="--f: {pct(recipe.exposure_ev, -3, 3)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.exposure_ev)}</code>
              </label>
            </div>
          {/if}
        </section>

        <section>
          <h2>Caption</h2>
          <textarea
            rows="2"
            placeholder="légende…"
            bind:value={caption}
            oninput={captionEdited}
          ></textarea>
        </section>

        <section>
          <h2>Export</h2>
          <label class="row">
            <span>TAILLE</span>
            <select bind:value={exportEdge} onchange={saveExportPrefs}>
              <option value={0}>Plein</option>
              <option value={4096}>4096</option>
              <option value={2048}>2048</option>
              <option value={1600}>1600</option>
              <option value={1024}>1024</option>
            </select>
          </label>
          <label class="row check">
            <span>BORDURE</span>
            <input type="checkbox" bind:checked={exportBorder} />
          </label>
          <button onclick={exportCurrent}>Exporter cette photo</button>
        </section>

        <!-- Section: Tonalité -->
        <section class="collapsible">
          <button class="section-toggle" onclick={() => tonalityOpen = !tonalityOpen}>
            <span>TONALITÉ</span>
            <span class="chevron">{tonalityOpen ? "▼" : "▶"}</span>
          </button>
          {#if tonalityOpen}
            <div class="section-content">
              <label class="row">
                <span>TIRAGE</span>
                <!-- Inverted control only (see the dev-panel `slider` snippet):
                     right = brighter. More enlarger exposure physically darkens
                     the print, so the stored value is the negation of what's
                     shown; the recipe and engine math are unchanged. -->
                <input
                  type="range"
                  min="-3"
                  max="3"
                  step="0.05"
                  value={-(recipe?.print_exposure_ev ?? 0)}
                  style="--f: {pct(-(recipe?.print_exposure_ev ?? 0), -3, 3)}"
                  oninput={(e) => {
                    if (recipe) recipe.print_exposure_ev = -Number(e.currentTarget.value);
                    edited(true);
                  }}
                  onchange={(e) => {
                    if (recipe) recipe.print_exposure_ev = -Number(e.currentTarget.value);
                    edited(false);
                  }}
                />
                <code>{fmt(-(recipe?.print_exposure_ev ?? 0))}</code>
              </label>
              <label class="row">
                <span>BLANCS</span>
                <input type="range" min="-1" max="1" step="0.05" bind:value={recipe.whites} style="--f: {pct(recipe.whites, -1, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.whites)}</code>
              </label>
              <label class="row">
                <span>HAUTES LUM.</span>
                <input type="range" min="-1" max="1" step="0.05" bind:value={recipe.highlights} style="--f: {pct(recipe.highlights, -1, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.highlights)}</code>
              </label>
              <label class="row">
                <span>TONS MOYENS</span>
                <input type="range" min="-1" max="1" step="0.05" bind:value={recipe.midtones} style="--f: {pct(recipe.midtones, -1, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.midtones)}</code>
              </label>
              <label class="row">
                <span>OMBRES</span>
                <input type="range" min="-1" max="1" step="0.05" bind:value={recipe.shadows} style="--f: {pct(recipe.shadows, -1, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.shadows)}</code>
              </label>
              <label class="row">
                <span>RÉDUC. HL</span>
                <input type="range" min="0" max="1" step="0.05" bind:value={recipe.rolloff} style="--f: {pct(recipe.rolloff, 0, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.rolloff)}</code>
              </label>
            </div>
          {/if}
        </section>

        <!-- Section: Film -->
        <section class="collapsible">
          <button class="section-toggle" onclick={() => filmOpen = !filmOpen}>
            <span>FILM & PAPIER</span>
            <span class="chevron">{filmOpen ? "▼" : "▶"}</span>
          </button>
          {#if filmOpen}
            <div class="section-content">
              <label class="row">
                <span>FILM</span>
                <select bind:value={recipe.film} onchange={() => edited()}>
                  {#each films as f}<option value={f.name}>{f.label}</option>{/each}
                </select>
              </label>
              <label class="row">
                <span>PAPIER</span>
                <select bind:value={recipe.paper} onchange={() => edited()}>
                  {#each papers as p}<option value={p.name}>{p.label}</option>{/each}
                </select>
              </label>
              <label class="row">
                <span>FILTRE Y</span>
                <input type="range" min="-30" max="30" step="1" bind:value={recipe.y_shift} style="--f: {pct(recipe.y_shift, -30, 30)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.y_shift)}</code>
              </label>
              <label class="row">
                <span>FILTRE M</span>
                <input type="range" min="-30" max="30" step="1" bind:value={recipe.m_shift} style="--f: {pct(recipe.m_shift, -30, 30)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.m_shift)}</code>
              </label>
            </div>
          {/if}
        </section>

        <!-- Section: Texture -->
        <section class="collapsible">
          <button class="section-toggle" onclick={() => textureOpen = !textureOpen}>
            <span>TEXTURE & GRAIN</span>
            <span class="chevron">{textureOpen ? "▼" : "▶"}</span>
          </button>
          {#if textureOpen}
            <div class="section-content">
              <label class="row">
                <span>GRAIN</span>
                <input type="range" min="0" max="1" step="0.05" bind:value={recipe.grain} style="--f: {pct(recipe.grain, 0, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.grain)}</code>
              </label>
              <label class="row">
                <span>HALATION</span>
                <input type="range" min="0" max="1" step="0.05" bind:value={recipe.halation} style="--f: {pct(recipe.halation, 0, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.halation)}</code>
              </label>
              <label class="row">
                <span>TAILLE HALO</span>
                <input type="range" min="0.5" max="1.5" step="0.05" bind:value={recipe.halation_size} style="--f: {pct(recipe.halation_size, 0.5, 1.5)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.halation_size)}</code>
              </label>
              <label class="row">
                <span>DIFFUSION</span>
                <input type="range" min="0" max="0.5" step="0.05" bind:value={recipe.diffusion} style="--f: {pct(recipe.diffusion, 0, 0.5)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.diffusion)}</code>
              </label>
              <label class="row">
                <span>NETTETÉ</span>
                <input type="range" min="0" max="1" step="0.05" bind:value={recipe.sharpen} style="--f: {pct(recipe.sharpen, 0, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.sharpen)}</code>
              </label>
            </div>
          {/if}
        </section>
      </aside>
    {/if}
  </div>
{/if}
<a href="/dev-panel" style="display: none;">Prerender Target</a>

{#if settingsOpen}
  <SettingsModal
    bind:preferences={preferences}
    onClose={() => (settingsOpen = false)}
    onChooseFolder={choosePreferenceFolder}
    onSave={saveSettings}
  />
{/if}

{#if fullscreen && view[sel]}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fullscreen-photo" onclick={exitFullscreen} role="presentation">
    <!-- The real single-photo viewer, so fullscreen is identical to dev mode.
         trackDevelopViewport is intentionally omitted here: it drives the dev
         render resolution off the underlying (grid/dev) viewport, and letting
         the fullscreen surface feed it would re-render the dev image at screen
         size. Fullscreen just shows the pre-developed `fullscreenUrl`. -->
    <DevelopView
      picked={view[sel].name}
      imgUrl={fullscreenUrl ?? undefined}
      useCanvas={false}
      zoomMode="frame"
      {developPhotoPercent}
    />
    {#if view[sel].rating}
      <span class="fullscreen-rating">{stars(view[sel].rating)}</span>
    {/if}
  </div>
{/if}

{#if shortcutsOpen}
  <ShortcutsModal onClose={() => (shortcutsOpen = false)} />
{/if}

<style>
  .render-badge {
    position: absolute;
    top: 20px;
    right: 20px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    background: rgba(18, 18, 18, 0.85);
    backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 999px;
    color: rgba(255, 255, 255, 0.9);
    font-size: 11px;
    letter-spacing: 0.04em;
    z-index: 100;
    pointer-events: none;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
  .render-spinner {
    width: 10px;
    height: 10px;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #ffffff;
    border-radius: 50%;
    animation: render-spin 0.6s linear infinite;
  }
  @keyframes render-spin {
    to {
      transform: rotate(360deg);
    }
  }

  /* ================= grille (cull) ================= */
  .cull {
    height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
    z-index: 1;
    overflow: hidden;
  }
  .window-controls {
    position: fixed;
    top: 15px;
    left: 18px;
    z-index: 100;
    display: flex;
    gap: 8px;
    transition: opacity var(--duration-standard) var(--ease-soft);
  }
  .window-controls.dev,
  .window-controls.fullscreen {
    z-index: 1001;
  }
  /* Auto-hide in Develop/Fullscreen mode when mouse leaves app window */
  .window-controls.away {
    opacity: 0;
    pointer-events: none;
  }

  /* Default resting state across all modes: subtle, muted dots */
  .window-controls button {
    all: unset;
    box-sizing: border-box;
    position: relative;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    cursor: default;
    background: var(--color-foreground, #8a8a8a);
    opacity: 0.35;
    transition: opacity var(--duration-instant) var(--ease-soft), background-color var(--duration-instant) var(--ease-soft);
  }

  /* When hovering anywhere over the traffic light group: reveal full system colors & glyphs */
  .window-controls:has(button:hover) button {
    opacity: 1;
  }
  .window-controls:has(button:hover) .window-close {
    background: #ff5f57;
  }
  .window-controls:has(button:hover) .window-minimize {
    background: #febc2e;
  }
  .window-controls:has(button:hover) .window-zoom {
    background: #28c840;
  }

  .window-controls:has(button:hover) button::after {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: rgb(0 0 0 / 55%);
    font-family: sans-serif;
    font-size: 8px;
    font-weight: 700;
    line-height: 1;
  }
  .window-controls:has(button:hover) .window-close::after {
    content: "×";
  }
  .window-controls:has(button:hover) .window-minimize::after {
    content: "−";
  }
  .window-controls:has(button:hover) .window-zoom::after {
    content: "+";
  }
  .fullscreen-photo {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-background);
    cursor: zoom-out;
  }
  .fullscreen-rating {
    position: fixed;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    color: var(--color-accent);
    font-size: 12px;
    letter-spacing: 0.08em;
  }
  .settings-overlay {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 22%);
    backdrop-filter: blur(8px);
  }
  .settings-panel {
    display: flex;
    flex-direction: column;
    width: min(620px, calc(100vw - 48px));
    height: min(720px, calc(100vh - 48px));
    overflow: hidden;
    border: 1px solid var(--color-border);
    border-radius: 14px;
    background: var(--color-surface-high);
    box-shadow: var(--shadow-xl);
  }
  .settings-header,
  .settings-footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 20px;
    border-color: var(--color-border);
  }
  .settings-header {
    border-bottom: 1px solid var(--color-border);
  }
  .settings-header h2 {
    flex: 1;
    margin: 0;
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    letter-spacing: 0.12em;
  }
  .settings-header button,
  .settings-footer button,
  .settings-picker button {
    all: unset;
    cursor: pointer;
  }
  .settings-scroll {
    flex: 1;
    overflow: auto;
    padding: 22px;
  }
  .settings-scroll fieldset {
    display: grid;
    gap: 16px;
    margin: 0 0 28px;
    padding: 0;
    border: 0;
  }
  .settings-scroll legend,
  .settings-scroll label > span {
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.12em;
  }
  .settings-scroll legend {
    width: 100%;
    margin-bottom: 2px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--color-border);
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .settings-scroll label {
    display: grid;
    gap: 6px;
  }
  .settings-scroll input {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    padding: 7px 9px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--color-muted) 5%, transparent);
    color: var(--color-muted);
    font-family: var(--font-monospace, monospace);
    font-size: 11px;
  }
  .settings-scroll small {
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    font-size: 10px;
  }
  .settings-picker {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
  }
  .settings-picker button,
  .settings-footer button {
    padding: 6px 10px;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.1em;
  }
  .settings-footer {
    justify-content: flex-end;
    border-top: 1px solid var(--color-border);
  }
  .settings-footer .primary {
    background: var(--color-foreground);
    color: var(--color-background);
  }
  .cull > .body {
    flex: 1;
    min-height: 0;
    display: flex;
    position: relative;
    z-index: 1;
    overflow: hidden;
  }
  .focus-overlay {
    position: fixed;
    inset: 0;
    z-index: 0;
    pointer-events: none;
    background: rgba(0, 0, 0, 0.18);
    backdrop-filter: grayscale(1) blur(18px);
  }
  .import-hud {
    position: fixed;
    left: 50%;
    bottom: 1.2rem;
    transform: translateX(-50%);
    z-index: 60;
    display: flex;
    align-items: center;
    gap: 0.7rem;
    max-width: min(42rem, calc(100vw - 2rem));
    padding: 0.55rem 0.8rem;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: var(--color-surface-high);
    box-shadow: var(--shadow-lg);
    font-family: var(--font-monospace, monospace);
    font-size: 0.68rem;
  }
  .import-hud strong {
    color: var(--color-accent);
    text-transform: uppercase;
  }
  .import-hud em {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.7;
    font-style: normal;
  }
  .import {
    color: var(--color-accent);
    border-color: var(--color-accent);
  }
  .progress {
    font-family: var(--font-monospace, monospace);
    font-size: 0.65rem;
    color: var(--color-accent);
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
    gap: 0;
    position: relative;
  }
  .sidebar-peek {
    position: absolute;
    top: 48px;
    left: 8px;
    z-index: 19;
    filter: drop-shadow(0 6px 16px rgba(0, 0, 0, 0.24));
    animation: sidebar-peek-in var(--duration-fast) var(--ease-soft);
  }
  @keyframes sidebar-peek-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  /* ── The top rail — canvas-toned (the Swift rule: the grid dissolves
     into it), icons only, one 44px toolbar line across the window. */
  .rail {
    height: 44px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: nowrap;
    white-space: nowrap;
    gap: 12px;
    padding: 0 16px;
    position: relative;
    z-index: 20;
  }
  /* Sidebar hidden → the native traffic lights overlay here; start past them. */
  .rail.solo {
    padding-left: var(--window-controls-offset-content, 86px);
  }
  .rail :global(button) {
    white-space: nowrap;
    flex-shrink: 0;
  }
  .brand-cluster {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-right: 4px;
  }
  .chrome-btn {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 12px;
    height: 12px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .chrome-btn:hover {
    color: var(--color-foreground);
  }
  .chrome-btn.on {
    color: var(--color-accent);
  }
  .focus-glyph {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1px solid currentColor;
    position: relative;
  }
  .focus-glyph::after {
    content: "";
    position: absolute;
    inset: 2px;
    border-radius: 50%;
    border: 1px solid currentColor;
  }
  .focus-glyph.on::after {
    background: currentColor;
  }
  .wordmark {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .wordmark:hover {
    color: var(--color-foreground);
  }

  .rail-menu {
    position: relative;
    display: inline-flex;
    flex-shrink: 0;
  }
  .rail-btn {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .rail-btn:hover {
    color: var(--color-foreground);
  }
  /* Active filter = solid ink + a filled star (Swift star.fill). */
  .rail-btn.on {
    color: var(--color-foreground);
  }
  .rail-btn.on :global(svg path) {
    fill: currentColor;
  }
  .frame-count {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    flex-shrink: 0;
  }
  .rail-spacer {
    flex: 1;
    min-width: 12px;
  }
  .rail-action {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    flex-shrink: 0;
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    letter-spacing: 0.02em;
    line-height: 1;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .rail-action:hover:not(:disabled) {
    color: var(--color-foreground);
  }
  .rail-action:disabled {
    color: color-mix(in srgb, var(--color-foreground) 22%, transparent);
    cursor: default;
  }

  /* The rail menus — small elevated panels under their icon. */
  /* Below the rail (z 20) so the open menu stays clickable; above the grid. */
  .pop-backdrop {
    position: fixed;
    inset: 0;
    z-index: 10;
  }
  .popover {
    position: absolute;
    top: calc(100% + 8px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 40;
    min-width: 168px;
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .layout-pop {
    left: auto;
    right: 0;
    transform: none;
    min-width: 208px;
    gap: 6px;
  }
  .pop-row {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 8px;
    border-radius: var(--radius);
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: var(--color-foreground);
  }
  .pop-row:hover {
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
  }
  .context-backdrop {
    position: fixed;
    inset: 0;
    z-index: 70;
  }
  .photo-context-menu,
  .context-submenu-panel {
    position: fixed;
    z-index: 71;
    min-width: 220px;
    padding: 4px;
    border: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
    border-radius: 8px;
    background: color-mix(in srgb, var(--color-surface-high) 88%, transparent);
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35), 0 0 0 0.5px rgba(0, 0, 0, 0.15);
    backdrop-filter: blur(24px) saturate(180%);
    -webkit-backdrop-filter: blur(24px) saturate(180%);
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif;
  }
  .context-header {
    padding: 4px 8px 5px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    text-transform: uppercase;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .photo-context-menu button,
  .context-submenu-panel button {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    color: var(--color-foreground);
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif;
    font-size: 12px;
    line-height: 1.4;
    cursor: default;
  }
  .photo-context-menu button:hover,
  .context-submenu-panel button:hover {
    background: #007aff;
    color: #ffffff;
  }
  .photo-context-menu button.disabled,
  .context-submenu-panel button.disabled {
    opacity: 0.4;
    pointer-events: none;
  }
  .photo-context-menu kbd {
    font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif;
    font-size: 11px;
    opacity: 0.6;
    margin-left: 12px;
  }
  .photo-context-menu button:hover kbd {
    color: #ffffff;
    opacity: 0.9;
  }
  .context-divider {
    height: 1px;
    margin: 3px 4px;
    background: color-mix(in srgb, var(--color-border) 40%, transparent);
  }
  .context-rating-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 3px 8px;
  }
  .context-rating-label {
    font-size: 12px;
    color: color-mix(in srgb, var(--color-foreground) 75%, transparent);
  }
  .context-stars {
    display: flex;
    align-items: center;
    gap: 1px;
  }
  .context-stars .star-btn {
    all: unset;
    padding: 1px 3px;
    font-size: 12px;
    color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .context-stars .star-btn.active {
    color: #ffcc00;
  }
  .context-stars .star-btn:hover {
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
    color: #ffcc00;
  }
  .context-stars .star-clear {
    all: unset;
    padding: 1px 4px;
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
    margin-left: 2px;
    cursor: pointer;
  }
  .context-stars .star-clear:hover {
    background: rgba(255, 59, 48, 0.2);
    color: #ff3b30;
  }
  .submenu-arrow {
    font-size: 12px;
    opacity: 0.6;
    margin-left: 8px;
  }
  .context-submenu {
    position: relative;
  }
  .context-submenu-panel {
    display: none;
    position: absolute;
    top: -5px;
    left: calc(100% - 2px);
  }
  .context-submenu-panel.submenu-left {
    right: calc(100% - 2px);
    left: auto;
  }
  .context-submenu:hover .context-submenu-panel,
  .context-submenu:focus-within .context-submenu-panel {
    display: block;
  }
  .pop-divider {
    height: 1px;
    background: var(--color-border);
    margin: 4px 0;
  }
  .pop-label {
    font-family: var(--font-header, sans-serif);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    padding: 0 8px;
  }
  .pop-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 2px;
    padding: 0 4px;
  }
  .pop-grid.three {
    grid-template-columns: repeat(3, 1fr);
  }
  .pop-chip {
    all: unset;
    cursor: pointer;
    text-align: center;
    padding: 3px 0;
    border-radius: var(--radius);
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .pop-chip.on {
    background: var(--color-foreground);
    color: var(--color-background);
  }
  .pop-slider {
    width: auto;
    margin: 0 8px 4px;
    accent-color: var(--color-accent);
  }

  /* The live export chip — the one place a count rides the chrome in red. */
  .export-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .chip-thumb {
    width: 20px;
    height: 20px;
    object-fit: cover;
    border-radius: var(--radius-sm);
    flex-shrink: 0;
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--color-foreground) 15%, transparent);
  }
  .chip-label {
    font-family: var(--font-header, sans-serif);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .chip-count {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    color: var(--color-foreground);
  }
  .chip-bar {
    width: 40px;
    height: 2px;
    border-radius: 1px;
    background: color-mix(in srgb, var(--color-foreground) 14%, transparent);
    overflow: hidden;
  }
  .chip-fill {
    display: block;
    height: 100%;
    background: var(--color-accent);
    transition: width var(--duration-standard) var(--ease-soft);
  }
  .chip-stop {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-sm);
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .chip-stop:hover {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }

  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    text-align: center;
  }
  .empty h1 {
    font-family: var(--font-header, sans-serif);
    font-size: 2.4rem;
    letter-spacing: 0.24em;
    color: var(--color-accent);
    margin: 0 0 0.3rem;
  }
  .empty p {
    font-family: var(--font-monospace, monospace);
    font-size: 0.72rem;
    opacity: 0.6;
  }
  .empty p + p {
    margin-top: 0.5rem;
  }

  /* ================= develop ================= */
  .app {
    display: grid;
    grid-template-columns: 1fr 19.5rem;
    height: 100vh;
    position: relative;
    z-index: 1;
  }
  main {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.6rem;
    overflow: hidden;
  }
  figure {
    margin: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    position: relative;
  }
  .photo-mat {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    width: fit-content;
    height: fit-content;
    max-width: calc(100% - (var(--space) * 2));
    max-height: calc(100% - (var(--space) * 2));
    padding: var(--space);
    background: var(--color-photo-frame);
    border-radius: var(--radius);
    box-shadow: var(--shadow-raised);
  }
  @media (prefers-color-scheme: dark) {
    .photo-mat {
      padding: 0;
      background: transparent;
      box-shadow: none;
    }
  }
  :global([data-color-mode="dark"]) .photo-mat {
    padding: 0;
    background: transparent;
    box-shadow: none;
  }
  .photo-mat img {
    box-sizing: border-box;
    display: block;
    max-width: 100%;
    max-height: 100%;
    border-radius: var(--radius);
    box-shadow: var(--shadow-ring), var(--shadow-inset);
    transition: opacity var(--duration-instant);
  }
  .photo-mat img.dimmed {
    opacity: 0.75;
  }
  /* Graceful stand-in when the loupe image can't decode/load (NAS drop, a junk
     file that slipped in, a moved original) — a quiet glyph + the name, never
     the browser's broken-image icon. */
  .photo-mat-failed {
    min-width: 14rem;
    min-height: 10rem;
  }
  .photo-fallback {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.55rem;
    padding: 2rem;
    color: var(--color-foreground);
    opacity: 0.5;
    text-align: center;
  }
  .fallback-name {
    font-family: var(--font-monospace, monospace);
    font-size: 0.8rem;
    word-break: break-all;
  }
  .fallback-hint {
    font-size: 0.68rem;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    opacity: 0.7;
  }
  /* Zoom "fill" — the framed look, just larger: we only drop the
     developPhotoPercent cap (via the empty inline style), so the photo grows to
     the mat's own max (calc(100% - space*2)). The mat, its breathing white
     space and the raised shadow all stay. Nothing to override here. */
  /* Zoom "actual" — 1:1 native pixels; pan by scrolling when it overflows.
     `safe` centering keeps the top-left reachable instead of clipping it. */
  main.zoom-actual {
    padding: 0;
    overflow: auto;
    justify-content: safe center;
    align-items: safe center;
    cursor: grab;
  }
  main.zoom-actual.panning {
    cursor: grabbing;
  }
  main.zoom-actual img {
    user-select: none;
    -webkit-user-drag: none;
  }
  main.zoom-actual figure {
    /* Grow with the image so `main` can scroll to its edges, but never shrink
       below the viewport so a small photo still centers. */
    width: max-content;
    height: max-content;
    min-width: 100%;
    min-height: 100%;
  }
  main.zoom-actual .photo-mat {
    max-width: none;
    max-height: none;
    padding: 0;
    background: none;
    border-radius: 0;
    box-shadow: none;
  }
  main.zoom-actual img {
    max-width: none;
    max-height: none;
    border-radius: 0;
    box-shadow: none;
  }
  .preview-status {
    position: absolute;
    left: 50%;
    bottom: 1rem;
    transform: translateX(-50%);
    margin: 0;
    padding: 0.45rem 0.7rem;
    border: 1px solid var(--color-border);
    border-radius: 999px;
    background: var(--color-surface-high);
    box-shadow: var(--shadow-lg);
    color: var(--color-foreground);
    font-family: var(--font-monospace, monospace);
    font-size: 0.68rem;
    white-space: nowrap;
  }

  /* the panel is a floating card, not a flat column */
  aside {
    margin: 2.2rem 0.9rem 0.9rem 0;
    padding: 1.1rem 1rem;
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    border-radius: 10px;
    box-shadow: var(--shadow-lg);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1.15rem;
    font-size: 0.8rem;
  }
  .open {
    align-self: flex-start;
  }
  .file {
    font-family: var(--font-monospace, monospace);
    font-size: 0.66rem;
    opacity: 0.6;
    margin: 0;
    overflow-wrap: anywhere;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  aside h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.85rem;
    border-bottom: none;
    padding: 0;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    font-weight: 700;
    margin: 0 0 0.15rem;
    opacity: 0.75;
  }
  .row {
    display: grid;
    grid-template-columns: 6.8rem 1fr 2.6rem;
    align-items: center;
    gap: 0.5rem;
  }
  .row span {
    font-family: var(--font-monospace, monospace);
    font-size: 0.6rem;
    letter-spacing: 0.04em;
    opacity: 0.7;
    white-space: nowrap;
    overflow: visible;
  }
  .row select {
    grid-column: 2 / 4;
    width: 100%;
  }
  .row.check input {
    justify-self: start;
    accent-color: var(--color-accent);
  }
  .row code {
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    text-align: right;
    opacity: 0.8;
    /* The grid already pins this column, so the track can't resize; this just
       stops the digits themselves from jittering as the value changes. */
    font-variant-numeric: tabular-nums;
  }
  input[type="range"] {
    width: 100%;
    accent-color: var(--color-accent);
  }
  textarea {
    font-family: var(--font-text, sans-serif);
    font-size: 0.75rem;
    color: var(--color-foreground);
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.4rem 0.5rem;
    resize: vertical;
  }
  .status {
    font-family: var(--font-monospace, monospace);
    opacity: 0.6;
  }

  .story-composer {
    flex: 1;
    min-height: 0;
    height: 100%;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .composer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-high);
  }
  .composer-header h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 1rem;
    letter-spacing: 0.12em;
    margin: 0;
    color: var(--color-foreground);
  }
  .composer-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .dest-select {
    background: var(--color-surface-low);
    color: var(--color-foreground);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.4rem 0.8rem;
    font-size: 0.72rem;
    font-family: var(--font-header, sans-serif);
    cursor: pointer;
    outline: none;
  }
  .publish-btn {
    color: var(--color-accent);
    border-color: var(--color-accent);
  }
  .composer-status {
    font-family: var(--font-monospace, monospace);
    font-size: 0.68rem;
    color: color-mix(in srgb, var(--color-foreground) 60%, transparent);
    max-width: 22rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .composer-status.ok {
    color: var(--color-accent);
  }

  /* Collapsible sections in develop panel */
  .collapsible {
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-surface-low);
    overflow: hidden;
  }
  .section-toggle {
    all: unset;
    cursor: pointer;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.6rem 0.8rem;
    font-family: var(--font-header, sans-serif);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--color-foreground);
    opacity: 0.85;
    background: var(--color-surface-high);
    border-bottom: 1px solid var(--color-border);
    user-select: none;
  }
  .section-toggle:hover {
    opacity: 1;
    background: var(--color-surface-low);
  }
  .section-toggle .chevron {
    font-size: 0.55rem;
    opacity: 0.5;
  }
  .section-content {
    padding: 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  /* Shortcuts Overlay Panel */
  .shortcuts-overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(12px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
  }
  .shortcuts-panel {
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    width: 100%;
    max-width: 38rem;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }
  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--color-border);
    padding-bottom: 0.75rem;
  }
  .panel-header h2 {
    font-family: var(--font-header, sans-serif);
    font-size: 1rem;
    letter-spacing: 0.12em;
    margin: 0;
    color: var(--color-accent);
  }
  .close-btn {
    all: unset;
    cursor: pointer;
    font-size: 0.85rem;
    opacity: 0.5;
    padding: 0.2rem;
  }
  .close-btn:hover {
    opacity: 0.9;
  }
  .shortcuts-grid {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    max-height: 28rem;
    overflow-y: auto;
  }
  .shortcut-group {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
  }
  .shortcut-group h3 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.68rem;
    letter-spacing: 0.1em;
    opacity: 0.45;
    margin: 0 0 0.15rem 0;
  }
  .shortcut-row {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    font-size: 0.72rem;
  }
  .shortcut-row kbd {
    font-family: var(--font-monospace, monospace);
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.15rem 0.4rem;
    font-size: 0.65rem;
    color: var(--color-accent);
    min-width: 1.8rem;
    text-align: center;
  }
  .shortcut-row span {
    opacity: 0.85;
  }

  /* Overlays & Modals */
  .overlay-modal {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(8px);
    z-index: 1000;
    display: flex;
    justify-content: center;
    align-items: center;
  }
  .modal-card {
    background: var(--color-surface-high);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    width: 32rem;
    max-width: 90vw;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }
  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-surface-low);
  }
  .modal-header h3 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.82rem;
    letter-spacing: 0.12em;
    margin: 0;
    color: var(--color-foreground);
  }
  .close-btn {
    background: none;
    border: none;
    color: var(--color-foreground);
    cursor: pointer;
    font-size: 1rem;
    opacity: 0.6;
  }
  .close-btn:hover {
    opacity: 1;
  }
  .modal-body {
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    max-height: 24rem;
    overflow-y: auto;
  }
  .modal-body textarea {
    width: 100%;
    height: 15rem;
    background: var(--color-background);
    color: var(--color-foreground);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 1rem;
    font-family: var(--font-monospace, monospace);
    font-size: 0.82rem;
    resize: none;
    outline: none;
  }
  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 1rem;
    padding: 1rem 1.5rem;
    border-top: 1px solid var(--color-border);
    background: var(--color-surface-low);
  }
  .modal-footer button {
    padding: 0.4rem 1.2rem;
    border-radius: var(--radius-sm);
    font-size: 0.72rem;
    font-family: var(--font-header, sans-serif);
    background: var(--color-accent);
    color: #fff;
    border: none;
    cursor: pointer;
  }
  .modal-footer button.secondary {
    background: var(--color-surface-low);
    color: var(--color-foreground);
    border: 1px solid var(--color-border);
  }

  /* Render queue list */
  .queue-list {
    gap: 0.8rem;
  }
  .queue-cancel {
    margin-left: auto;
    margin-right: 0.75rem;
    padding: 0.3rem 0.65rem;
    border: 1px solid color-mix(in srgb, var(--color-accent) 42%, var(--color-border));
    border-radius: 999px;
    background: transparent;
    color: var(--color-accent);
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    cursor: pointer;
  }
  .queue-item {
    background: var(--color-surface-low);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    padding: 0.8rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .queue-item.active {
    border-color: color-mix(in srgb, var(--color-accent) 42%, var(--color-border));
  }
  .queue-item-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.68rem;
    font-family: var(--font-monospace, monospace);
  }
  .queue-time {
    opacity: 0.5;
  }
  .queue-name {
    font-weight: bold;
    color: var(--color-foreground);
  }
  .queue-progress-bar {
    width: 100%;
    height: 4px;
    background: var(--color-background);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--color-accent);
    transition: width 0.2s var(--ease-standard);
  }
  .queue-status {
    display: flex;
    justify-content: space-between;
    font-size: 0.62rem;
    font-family: var(--font-header, sans-serif);
    opacity: 0.7;
  }
  .queue-phase {
    font-family: var(--font-monospace, monospace);
    font-size: 0.6rem;
    color: color-mix(in srgb, var(--color-foreground) 48%, transparent);
  }
  .queue-phase.error {
    color: var(--color-accent);
  }
  .empty-queue {
    text-align: center;
    font-size: 0.72rem;
    font-family: var(--font-monospace, monospace);
    opacity: 0.5;
    padding: 2rem 0;
  }

  /* Editor select in dev panel */
  .editor-select-container {
    margin: 0.5rem 0;
  }
  .editor-select {
    width: 100%;
    background: var(--color-surface-low);
    color: var(--color-foreground);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0.3rem 0.5rem;
    font-size: 0.68rem;
    font-family: var(--font-header, sans-serif);
    outline: none;
    cursor: pointer;
  }
</style>
