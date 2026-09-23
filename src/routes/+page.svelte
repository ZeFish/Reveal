<script>
  import { tick, untrack } from "svelte";
  import { thumbUrl } from "$lib/thumbUrl.js";
  import { placePalette } from "$lib/palettePlacement.js";
  import { session, openFolderSession } from "$lib/session.js";
  import {
    library, beginOpen, appendFrames, refreshFrames, clearFrames, refreshLoadedFrames, leaveFolder,
    setRoots, setDirs, setLoading, dirLabel,
  } from "$lib/library.svelte.js";
  import {
    activity, notify, hold, dismiss,
    startActivity, updateActivity, setActive, releaseActive, setQueueOpen, setProgress, patchProgress, advanceProgress,
  } from "$lib/activity.svelte.js";
  import {
    selection, positionIn, anchorIn, focusAt, selectOnly, selectGridItem,
    selectRange, toggle as toggleSelected, selectAll, clearSelection,
    setSelection, setAnchor, selectedFrames,
  } from "$lib/selection.svelte.js";
  import { invoke } from "@tauri-apps/api/core";
  import { listen as tauriListen, emit } from "@tauri-apps/api/event";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { getCurrentWindow, currentMonitor, availableMonitors } from "@tauri-apps/api/window";
  import { LogicalPosition } from "@tauri-apps/api/dpi";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";
  import Sidebar from "@modules/sidebar/Sidebar.svelte";
  import { APPLE_PHOTOS_ROOT, findPhotoCollection } from "@modules/sidebar/applePhotosTree.js";
  import { reloadPhotoPages, restorePhotoSelection } from "@modules/sidebar/applePhotosBrowsing.js";
  import CullView from "@modules/culling/CullView.svelte";
  import DevelopView from "@modules/develop/DevelopView.svelte";
  import DevelopPanel from "@modules/develop/DevelopPanel.svelte";
  import StoryView from "@modules/story/StoryView.svelte";
  import { parseStory, serializeStory } from "$lib/story.js";
  import ShortcutsModal from "@modules/modals/ShortcutsModal.svelte";
  import RenderQueueModal from "@modules/modals/RenderQueueModal.svelte";
  import TaskIndicator from "@modules/modals/TaskIndicator.svelte";
  import Toast from "@modules/modals/Toast.svelte";
  import NotificationStack from "@modules/modals/NotificationStack.svelte";
  import ContextMenu from "@modules/menus/ContextMenu.svelte";
  import Dropdown from "@stnd/ui/Dropdown.svelte";
  import DropdownItem from "@stnd/ui/DropdownItem.svelte";
  import DropdownSeparator from "@stnd/ui/DropdownSeparator.svelte";
  import Popover from "@stnd/ui/Popover.svelte";
  import Dialog from "@stnd/ui/Dialog.svelte";
  import Alert from "@stnd/ui/Alert.svelte";
  import { AppController } from "$lib/controllers/AppController.js";
  import { extractGardenUrl } from "$lib/story.js";
  import { storyTheme, colorScheme, updateStoryTheme, contrastInk, getFontFamilyWithFallback, currentThemeFontPackages, currentThemeId } from "$lib/story-theme.svelte.js";
  import { loadFontPackages, measureThemeTokens } from "$lib/app-theme.js";

  // ---- shared shapes (plain-JS JSDoc typing — no runtime effect) -----------------
  /** A catalogue frame row, as returned by `index_frames` / `list_dir`. */
  /** @typedef {Object} Frame
   * @property {string} path
   * @property {string} name
   * @property {number} rating
   * @property {number} [width] as seen, sensor rotation applied — absent until rescanned
   * @property {number} [height]
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
  /** One entry in the activity queue — the single place every long-running
   * operation (import, export, culling, publish, move, develop-settings
   * sync) reports its progress, instead of each keeping its own ad-hoc
   * display. The top-right indicator shows an aggregate across every
   * "running" entry; the activity panel (old RenderQueueModal) lists them
   * individually, live ones and history together.
   * @typedef {Object} Activity
   * @property {string} id
   * @property {string} kind e.g. "import" | "export" | "cull" | "publish" | "move" | "develop"
   * @property {string} label
   * @property {string} current
   * @property {number} done
   * @property {number} total
   * @property {string} phase
   * @property {string} status "running" | "completed" | "failed" | "cancelled"
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
   * @property {number} renderMs
   * @property {Uint8ClampedArray<ArrayBuffer>} rgba
   */
  /**
   * @typedef {Window & typeof globalThis & {
   *   __log?: (...args: any[]) => void;
   *   currentMonitor?: () => Promise<any> | any;
   * }} RevealWindow
   */

  const PREVIEW_PX = 2048;
  // Live-drag proxy resolution, for the engines that still need one. It
  // existed because the per-pixel CPU path couldn't keep up frame to frame;
  // Rapid on the GPU renders 2048px in ~24ms, so that engine now drags at
  // full resolution and never shows a soft proxy at all (see liveRenderPx).
  const DRAG_PX = 768;
  // Set from the gpu_available command at startup. Only Rapid has a Reveal
  // GPU path — Spektra runs on spektrafilm-gpu's own wgpu backend already
  // and is still ~2.5s a frame, which is the spectral simulation's own cost,
  // so it keeps the proxy regardless.
  let gpuAvailable = $state(false);
  /** Resolution for a render: full unless a proxy still buys something.
   * @param {boolean} live */
  function liveRenderPx(live) {
    if (!live) return PREVIEW_PX;
    return developEngine === "rapid" && gpuAvailable ? PREVIEW_PX : DRAG_PX;
  }

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

  /** @type {"cull" | "dev"} */
  // Restored from the last session rather than hardcoded to the grid.
  //
  // Starting in "cull" always mounted the grid, which fires a thumb request
  // per visible cell — ~120 of them — and only THEN did the session restore
  // switch to Develop. On a NAS that is a hundred reads nobody asked for,
  // competing for bandwidth with the one photo being opened. If the last
  // session was in Develop, the grid never mounts at all. If the restore
  // finds no photo to reopen, `openDir` falls back to the grid anyway.
  // Validated, not cast: localStorage is arbitrary text, and an unknown mode
  // would mount neither surface and leave an empty window with no way out.
  let currentMode = $state(/** @type {"dev" | "cull"} */ (session.lastMode() ?? "cull"));
  // Editorial is a display filter on the grid, not a destination — flipping it
  // never changes `currentMode`. On, the grid's WYSIWYG rendering (StoryView)
  // replaces the dense grid: same photos, same interactions, laid out and
  // filtered exactly like the published page will read.
  let previewFilter = $state(false);
  // Space is a "loupe" — the single-photo canvas without the dev panel. While a
  // space-look is active we suppress the panel window regardless of the stored
  // layouts.dev.devPanel preference, so quick looks never pop (or steal focus
  // to) the panel. Any deliberate develop entry (d, click a photo, ⇧D) clears it.
  let spaceLook = $state(false);
  // Single-photo zoom cycle (Z / ⇧Z): the framed look with white space around
  // it → the photo filling the viewport edge-to-edge → 1:1 actual pixels → back.
  const ZOOM_CYCLE = ["frame", "fill", "actual"];
  let zoomMode = $state("frame"); // one of ZOOM_CYCLE
  let fullscreen = $state(false);
  /** @type {string | null} */ let fullscreenUrl = $state(null);
  let fullscreenRequest = 0;
  // Pointer/focus presence for the window — drives focus mode AND the
  // fullscreen traffic-light hint, which fades out when the mouse leaves.
  let pointerInside = $state(false);
  // The "frame" zoom's own size, as a % of the viewport — used to be an
  // auto golden-ratio formula off developViewport; now a plain slider in
  // DevTab (Francis: "so we can choose... the photo size"). >100 lets the
  // photo outgrow the frame on purpose — see the loupe/pan split below.
  let developPhotoPercent = $state(90);
  /** @type {Record<"cull" | "dev", Record<string, boolean>>} */
  let layouts = $state({
    cull: { sidebar: true, focus: false, devPanel: false },
    // detached: the Develop panel opens as its own OS window (the old
    // default). Docked (the new default) renders it inline instead — same
    // component (DevelopPanel.svelte), no IPC, just a different host.
    dev: { sidebar: false, focus: false, devPanel: true, detached: false }
  });
  // The docked DevelopPanel's own active tab, mirrored up here so
  // DevelopView knows when Crop is open (shows the crop grid/rotation
  // preview only then, not whenever a non-"original" aspect is just
  // sitting in the recipe from an earlier session).
  let dockedActiveTab = $state("dev");
  // RAW, not deep-reactive: the contact sheet holds up to ~22k frames, and a
  // plain $state would wrap every element in a Proxy — then any full-array pass
  // (view's sort, selectedFrames' filter) fires millions of proxy traps and
  // freezes the folder open. Raw means only reassigning `frames` is reactive,
  // so in-place edits (rating, previewVersion) reassign with `frames = [...frames]`.
  let applePhotosSupported = $state(false);
  let applePhotosActive = $state(false);
  let applePhotosBusy = $state(false);
  let applePhotosConnecting = $state(false);
  let applePhotosLoaded = $state(false);
  let applePhotosAlbum = $state("");
  /** @type {import('@modules/sidebar/applePhotosTree.js').PhotoCollection[]} */
  let applePhotosAlbums = $state([]);
  /** @type {number | null} */
  let applePhotosLibraryTotal = $state(null);
  /** @type {Promise<boolean> | null} */
  let applePhotosConnection = null;
  let applePhotosTotal = $state(0);
  let applePhotosOffset = $state(0);
  let applePhotosRequest = 0;
  /** @type {{phase: string, name: string, error?: string} | null} */
  let applePhotosTransfer = $state(null);
  const applePhotosLibrary = $derived({
    supported: applePhotosSupported, active: applePhotosActive,
    busy: applePhotosBusy || applePhotosConnecting, loaded: applePhotosLoaded,
    album: applePhotosAlbum, albums: applePhotosAlbums, total: applePhotosLibraryTotal,
  });

  $effect(() => {
    if (!isTauri) return;
    invoke("apple_photos_status", { authorize: false })
      .then((result) => { applePhotosSupported = result.supported; })
      .catch((error) => { notify(`Could not check Apple Photos availability: ${error}`); });
    const unlisten = listen("apple-photos-transfer", ({ payload }) => {
      applePhotosTransfer = payload;
      if (payload.phase === "error") hold(`Apple Photos: ${payload.error}`);
    });
    return () => { unlisten.then((stop) => stop()); };
  });

  /**
   * Disclosure can connect the catalogue without changing the current grid.
   * Startup restoration checks access but must never prompt for permission.
   * @param {boolean} authorize
   * @param {boolean} refresh
   */
  async function loadApplePhotosCollections(authorize = true, refresh = false) {
    if (applePhotosConnection) {
      const connected = await applePhotosConnection;
      // A user click can arrive while the non-prompting startup check runs.
      // Don't consume that deliberate connection attempt with a false result.
      if (!connected && authorize) return loadApplePhotosCollections(authorize, refresh);
      return connected;
    }
    applePhotosConnecting = true;
    applePhotosConnection = (async () => {
      const access = await invoke("apple_photos_status", { authorize });
      if (!["authorized", "limited"].includes(access.authorization)) {
        applePhotosAlbums = [];
        applePhotosLoaded = false;
        applePhotosLibraryTotal = null;
        if (applePhotosActive) clearFrames();
        if (!authorize) return false;
        throw new Error("Allow Reveal in System Settings > Privacy & Security > Photos, then click Apple Photos again.");
      }
      if (refresh || !applePhotosLoaded) {
        const collection = await invoke("apple_photos_albums");
        applePhotosAlbums = collection.albums;
        applePhotosLibraryTotal = collection.total;
        applePhotosLoaded = true;
      }
      return true;
    })();
    try {
      return await applePhotosConnection;
    } finally {
      applePhotosConnection = null;
      applePhotosConnecting = false;
    }
  }

  async function connectApplePhotos() {
    try {
      dismiss();
      await loadApplePhotosCollections();
    } catch (error) {
      hold(`Could not connect Apple Photos: ${error}`);
    }
  }

  async function refreshApplePhotos() {
    if (applePhotosBusy || applePhotosConnecting) return;
    if (applePhotosActive) {
      await openApplePhotos(applePhotosAlbum, { refresh: true });
      return;
    }
    try {
      dismiss();
      await loadApplePhotosCollections(true, true);
    } catch (error) {
      hold(`Could not refresh Apple Photos: ${error}`);
    }
  }

  /**
   * @param {string} album A PhotoKit album OR collection-list folder ID.
   * @param {{authorize?: boolean, refresh?: boolean}} options
   */
  async function openApplePhotos(album = "", { authorize = true, refresh = false } = {}) {
    const request = ++applePhotosRequest;
    let preserve = applePhotosActive && applePhotosAlbum === album;
    const previous = {
      selected: new Set(selection.paths), focus: view[sel]?.path,
      anchor: view[selectionAnchor]?.path, index: sel,
    };
    const loadedCount = library.frames.length;
    const descending = sortDesc;
    applePhotosBusy = true;
    dismiss();
    try {
      if (!await loadApplePhotosCollections(authorize, refresh) || request !== applePhotosRequest) return;
      // A removed collection must not leave the active row/grid orphaned after
      // refresh or relaunch. The catalogue header is always a valid fallback.
      if (album && !findPhotoCollection(applePhotosAlbums, album)) {
        album = "";
        preserve = false;
      }
      /** @type {(offset: number) => Promise<{frames: Frame[], total: number, next: number}>} */
      const readPage = (offset) => invoke("apple_photos_list", { album: album || null, offset, descending });
      const page = await reloadPhotoPages(
        readPage,
        {
          minimumCount: preserve ? loadedCount : 0,
          retainedPaths: preserve ? [...previous.selected, previous.focus, previous.anchor,
            currentMode === "dev" && photoPath?.startsWith("apple-photos://") ? photoPath : null] : [],
          isCurrent: () => request === applePhotosRequest,
        },
      );
      if (!page || request !== applePhotosRequest) return;
      applePhotosAlbum = album;
      applePhotosActive = true;
      applePhotosTotal = page.total;
      applePhotosOffset = page.next;
      const open = beginOpen({});
      open.commit(page.frames);
      open.finish();
      if (preserve) {
        const restored = restorePhotoSelection(view, previous);
        setSelection(restored.selected, view[restored.anchor]?.path ?? null);
        focusAt(view, restored.index);
        // Keep layout/filter, active Develop photo/recipe and scroll. PhotoGrid
        // keeps a moved focused row visible using its existing virtual geometry.
      } else {
        minRating = 0;
        filterStory = false;
        previewFilter = false;
        storySet = new Set();
        focusAt(view, 0);
        selectOnly(view, 0);
        currentScrollTop = 0;
      }
      session.setLastDirectory(APPLE_PHOTOS_ROOT + album);
      if (!preserve) await switchMode("cull");
    } catch (error) {
      if (request === applePhotosRequest) hold(`Could not open Apple Photos: ${error}`);
    } finally {
      if (request === applePhotosRequest) applePhotosBusy = false;
    }
  }

  async function loadMoreApplePhotos() {
    if (applePhotosBusy || !applePhotosActive) return;
    const request = applePhotosRequest;
    applePhotosBusy = true;
    try {
      const page = await invoke("apple_photos_list", {
        album: applePhotosAlbum || null, offset: applePhotosOffset, descending: sortDesc,
      });
      if (request !== applePhotosRequest) return;
      const known = new Set(library.frames.map((frame) => frame.path));
      appendFrames(page.frames);
      applePhotosOffset = page.next;
      applePhotosTotal = page.total;
    } catch (error) {
      if (request === applePhotosRequest) hold(`Could not load more photos: ${error}`);
    } finally {
      if (request === applePhotosRequest) applePhotosBusy = false;
    }
  }

  function leaveApplePhotos() {
    applePhotosRequest += 1;
    applePhotosActive = false;
    applePhotosBusy = false;
  }

  async function cancelApplePhotosTransfer() {
    try {
      await invoke("apple_photos_cancel");
    } catch (error) {
      hold(`Could not cancel photo download: ${error}`);
    }
  }

  /** @type {Recipe | null} */ let copiedRecipe = $state(null);
  /** @type {string | null} */ let importDir = $state(null); // chosen import folder (null → fall back to root)

  // Folder mood — the SAME story-theme tokens the Editorial/Garden preview
  // already reads from `<folder>/<folder-name>.md` frontmatter (the sidebar's
  // theme dropdown writes them via story_set_theme), now also driving Reveal's own working
  // chrome, not just the exported-preview canvas. A wedding folder and a
  // corporate-shoot folder can carry their own theme note and the app itself
  // shifts mood while you're in them — reusing the existing per-folder file
  // rather than inventing a second theming mechanism.
  $effect(() => {
    const dir = library.curDir;
    if (!isTauri || !dir) {
      updateStoryTheme({ darkBackground: null, darkAccent: null, fontHeader: null, fontText: null });
      return;
    }
    invoke("story_load_theme", { dir }).then((t) => {
      if (library.curDir !== dir) return; // folder changed again before this resolved
      updateStoryTheme({
        darkBackground: t.darkBackground,
        darkAccent: t.darkAccent,
        fontHeader: t.fontHeader,
        fontText: t.fontText,
      });
      // A saved theme only carries font NAMES ("Forrest") — the fontPackages
      // array that actually loads their @font-face CSS lives on the curated
      // theme entry, not the folder's note, so it has to be looked back up.
      const pkgs = currentThemeFontPackages();
      if (pkgs.length) loadFontPackages(pkgs);
    });
  });

  // The folder-theme override below only ever carried the DARK half of the
  // 8 theme tokens ThemeTokens actually models (darkBackground/darkAccent),
  // so a themed folder stayed visibly dark even after toggling the system
  // to light mode — "l" (toggleAppearance) flips the OS/webview's
  // prefers-color-scheme, which the framework's own :root rules follow
  // fine, but this override doesn't unless it ALSO knows which scheme is
  // active. story.rs's set_theme mirrors light_background = derived
  // foreground of dark_background, light_foreground = dark_background —
  // reproduced here with the same contrastInk helper rather than fetching
  // the light_* fields separately, since they're always that exact swap.
  // colorScheme.prefersDark is the shared singleton (story-theme.svelte.js)
  // — StoryView's own preview canvas needs the exact same signal, so it
  // isn't duplicated as a second local matchMedia listener here.
  let appThemed = $derived(!!storyTheme.darkBackground);
  let appBg = $derived(
    !storyTheme.darkBackground
      ? undefined
      : colorScheme.prefersDark
        ? storyTheme.darkBackground
        : contrastInk(storyTheme.darkBackground)
  );
  let appFg = $derived(
    !storyTheme.darkBackground
      ? undefined
      : colorScheme.prefersDark
        ? contrastInk(storyTheme.darkBackground)
        : storyTheme.darkBackground
  );
  let appAccent = $derived(storyTheme.darkAccent ?? undefined);
  let appFontHeader = $derived(storyTheme.fontHeader ? getFontFamilyWithFallback(storyTheme.fontHeader, true) : undefined);
  let appFontText = $derived(storyTheme.fontText ? getFontFamilyWithFallback(storyTheme.fontText, false) : undefined);
  // --font-ratio (heading modular scale) is independent of the background/
  // accent theme, so this is unconditional. Written here mainly to keep the
  // token present should any Reveal chrome start consuming --optical-ratio/
  // --scale-* later — today NONE of Reveal's own components size off those
  // (every .svelte file hardcodes px), so this override is currently inert
  // inside the app itself. Its real consumer is the published Garden page:
  // story_set_theme already writes font-ratio into the note's frontmatter,
  // which Garden's own site DOES render typography from.
  let appFontRatio = $derived(storyTheme.fontRatio ?? undefined);

  // Extends the same "app-wide mood" mechanism above (colors, fonts) to a
  // themed folder's actual shape — its curated theme's real CSS
  // (packages/themes/<id>/<id>.scss) also carries radius, derived the same
  // way colors are, which garden-themes.generated.json never modeled.
  // Reported live after this SAME fix already landed for the Editorial
  // preview: "back in grid and we lose it" — the grid never got it because
  // that fix only touched StoryView. measureThemeTokens (app-theme.js)
  // resolves it off a real, offscreen element carrying the theme's
  // data-theme attribute (this framework's own radius/color derivation
  // only resolves at :root by design, not at a nested scope) — see its own
  // comment for why reconfiguring that package-wide isn't the fix here.
  let appThemeId = $derived(currentThemeId());
  /** @type {Record<string, string>} */
  let appMeasuredRadius = $state({});
  $effect(() => {
    const id = appThemeId;
    if (!id) {
      appMeasuredRadius = {};
      return;
    }
    measureThemeTokens(id, ["--radius", "--radius-sm", "--radius-lg"]).then((tokens) => {
      if (appThemeId === id) appMeasuredRadius = tokens;
    });
  });

  let minRating = $state(0);
  let scanning = $state(false);
  /** @type {Card[]} */ let cards = $state([]);
  // The Swift export prefs, faithfully: long edge Plein/4096/2048/1600/1024
  // (default 2048), border toggle, and a remembered export folder ("" = the
  // Desktop, resolved Rust-side) — no dialog on every export.
  let exportEdge = $state(2048);
  let exportBorder = $state(false);
  let exportFolder = $state("");
  // True while the photo's own volume is unreachable. Not an error state —
  // the cache still answers — so it is shown as a quiet mark, not a toast: a
  // toast fades, and this condition lasts until the mount comes back.
  let sourceOffline = $state(false);
  /** @type {ReturnType<typeof setInterval> | undefined} */
  let sourceWatch;


  // The single way to tell the user something happened. Goes to the
  let autoImport = $state(false);
  // AI cull (walk-away card→cull→export): mirrors autoImport's hydrate-at-
  // boot pattern, but sourced from the generic preferences bag rather than
  // a dedicated ShellPrefs field (see SettingsModal's "AI CULL" section).
  // Two independent outcomes, not one master switch: marking picks into the
  // story (the `q` quick-collection) and exporting JPEGs to the Desktop can
  // each be on or off on their own.
  let aiCullMarkStory = $state(false);
  let aiCullExportDesktop = $state(false);
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
  // Sidebar Garden account row — null until boot resolves the stored key.
  /** @type {GardenAccount | null} */ let gardenAccount = $state(null);
  let preferences = $state({
    date_folders: "%Y/%Y-%m-%d",
    obsidian_enabled: false,
    vault: "",
    logs_folder: "Logs",
    export_folder: "",
    lut_folder: "",
    ai_cull_mark_story: false,
    ai_cull_export_desktop: false,
    ai_cull_target: 24,
    ai_provider: "anthropic",
    ai_api_key: "",
    ai_model: "",
    apple_photos_cache_limit_gib: 4,
    default_engine: "",
    app_theme: "reveal",
  });

  // Grid geometry + rail filters — the Swift model's columnsPref/cellAspect/
  // fillCells/marginScale/minStars/filterStory/sortMode, persisted together.
  let cols = $state(4);
  let marginScale = $state(1);
  let cellAspect = $state(1.5);
  let fillCells = $state(true);
  let filterStory = $state(false);
  let sortDesc = $state(false);
  let layoutMenuOpen = $state(false);
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
  const currentRating = $derived(library.frames.find((f) => f.path === photoPath)?.rating ?? 0);
  /** @type {string | null} */ let imgUrl = $state(null);
  /** @type {HTMLCanvasElement | null} */ let canvasEl = $state(null);
  let useCanvas = $state(false);
  // Bumped every time pump() paints fresh pixels into canvasEl via
  // putImageData — a plain counter DevelopView's histogram effect can
  // depend on. canvasEl.width/height alone isn't enough: two renders of the
  // same photo at the same output size (e.g. nudging exposure) leave those
  // DOM properties unchanged, so an effect keyed only on them never reruns
  // and the histogram goes stale after the very first Rapid-engine render.
  let canvasVersion = $state(0);
  /** @type {number | null} */ let renderAspect = $state(null); // width/height of the last canvas (rapid) render — reactive aspect for the loupe
  let imgFailed = $state(false); // loupe image couldn't decode/load (NAS drop, junk file…)
  /** @type {{r: Uint32Array, g: Uint32Array, b: Uint32Array, luma: Uint32Array} | null} */
  let histogram = $state(null); // computed by DevelopView off the same pixels it displays
  // Waveform/parade/vectorscope buffers from that same read. Stays in this
  // window only — see the `scopes` prop docblock in DevelopView.svelte for
  // why it isn't in sendDevStateToPanel's payload.
  let scopes = $state(null);
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
  let showCaption = $state(false);
  let caption = $state("");
  /** @type {string[]} */
  let tags = $state([]);
  // Docked Develop panel only — the detached one has its own separate copies
  // of these (its own onMount fetch, its own local publish button state).
  /** @type {Recipe | null} */
  let developDefaults = $state(null);
  let devPublishing = $state(false);
  let devPublishStatus = $state("");
  // Undo/redo for the develop recipe — scoped to whichever photo is
  // currently open (reset on openPhoto, not a global/cross-session history).
  // Each *settled* edit (edited(false) — every slider already distinguishes
  // this from the continuous edited(true) fired while dragging) pushes the
  // recipe as it was BEFORE that edit; undo/redo just replay the same
  // edited() commit path a normal change would, so render + disk save
  // happen for free.
  /** @type {Recipe[]} */
  let recipeUndoStack = $state([]);
  /** @type {Recipe[]} */
  let recipeRedoStack = $state([]);
  /** @type {Recipe | null} */
  let lastCommittedRecipe = null;
  let restoringRecipeHistory = false;
  const MAX_RECIPE_UNDO_STEPS = 50;
  /** @param {Recipe | null} r */
  const snapshotRecipe = (r) => (r ? JSON.parse(JSON.stringify(r)) : null);
  /** @type {Set<string>} */ let storySet = $state(new Set());
  /** @type {string | null} */ let liveUrl = $state(null);
  let storyContent = $state("");
  let gardenUrl = $derived(extractGardenUrl(storyContent) || liveUrl);

  /** @type {Record<string, number>} */ let scrollOffsets = $state({});
  /** @type {[string, string][]} */ let installedEditors = $state([]);
  let catalogContent = $state("");
  let catalogOpen = $state(false);

  // Cross-listener correlation: these backend event streams (publish-progress,
  // import-progress, cull-progress) fire many times over one logical
  // operation, so the id created when it starts needs to survive to update
  // the same activity entry on each subsequent event.
  /** @type {string | null} */ let publishTaskId = $state(null);
  /** @type {string | null} */ let cullTaskId = $state(null);
  let currentScrollTop = $state(0);
  /**
   * The open folder's remembered session — its scroll offset, its workflow
   * mode, and the only functions allowed to write them back.
   *
   * Holding it is what proves the folder's stored values have been READ.
   * Before, a boot-time `$effect` persisted `currentScrollTop`'s initial 0
   * over the real offset, which the loader then read back as 0: the grid
   * always reopened at the top however far you had scrolled. Nothing stated
   * an order between saving and loading, so it fell out of the reactivity
   * graph. Now there is nothing to write through until the read has happened.
   * @type {import("$lib/session.js").FolderSession | null}
   */
  let folderSession = $state(null);
  /** @type {PhotoMenu | null} */ let photoMenu = $state(null);

  $effect(() => {
    const d = library.dir;
    if (d && folderSession?.dir === d && currentMode === "cull") {
      scrollOffsets[d] = currentScrollTop;
      folderSession.saveScroll(currentScrollTop);
    }
  });

  // Session restore (openDir's restoreSession) reopens whichever photo was
  // in Develop last time the app quit — this is the write half.
  $effect(() => {
    if (currentMode === "dev" && photoPath) session.setLastPhoto(photoPath);
  });

  // ---- the backend event bus -----------------------------------------------------
  // Sixty-one of the app's sixty-two `listen()` registrations, in one effect
  // that runs once and now tears them all down again if it ever re-runs.
  $effect(() => {
    // Every backend listener registered here, so they can all be dropped
    // again. The effect runs once today — measured, not assumed — but 61
    // listeners with no teardown is a trap rather than a bug: the day a
    // reactive read appears at the top of this block, the effect re-runs and
    // every Rust event fires twice, then three times. The comment below
    // records that such a loop already happened here once.
    /** @type {Promise<() => void>[]} */
    const wired = [];
    /** @param {string} event @param {(payload: any) => void} handler */
    const on = (event, handler) => void wired.push(listen(event, handler));
    {
      const parsed = session.modeLayouts();
      if (parsed) {
        // Assign onto the PLAIN parsed object's values, not by reading
        // `layouts` here: that would make this boot effect depend on state it
        // also writes, a self-invalidating effect that re-ran forever and
        // hammered openDir/refreshDirs in a loop.
        Object.assign(layouts, parsed);
      }

      // Reveal always reopens into the contact sheet. The last workflow mode
      // remains available per folder, but never hijacks startup.
      currentMode = "cull";

      const saved = session.gridLayout();
      if (saved) layout = saved;
      {
        const x = session.exportPrefs();
        if (x) {
          if ([0, 4096, 2048, 1600, 1024].includes(x.edge)) exportEdge = x.edge;
          if (typeof x.border === "boolean") exportBorder = x.border;
          if (typeof x.folder === "string") exportFolder = x.folder;
        }
      }
      {
        const g = session.gridPrefs();
        if (g) {
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
      }
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
      // Decides whether a live drag renders full-res or through the proxy.
      gpuAvailable = await invoke("gpu_available").catch(() => false);
      // For the docked Develop panel's "reset to default" (⌥-click a slider
      // label) — the detached panel fetches its own copy in its own onMount;
      // this is the docked equivalent, fetched once, same as everything above.
      developDefaults = await invoke("default_recipe").catch(() => null);
      await refreshDirs();
      // Prefer the persisted path; fall back to the most recently indexed
      // directory when nothing was saved yet (fresh install / cleared
      // localStorage but an existing library). The autoload_path/
      // take_open_file check below only overrides this for an actual
      // deep-link/"open with" target — it used to redo this exact fallback
      // itself, indexing the same directory twice on every launch.
      const lastDir = session.lastDirectory() || library.dirs[library.dirs.length - 1]?.dir;
      if (lastDir) {
        // restoreSession: land back in Develop on whichever photo was open
        // when the app last quit, instead of always resetting to Grid.
        await openDir(lastDir, false, true);
      } else {
        currentMode = "cull";
      }
      // A seeded root with an empty index (fresh install, or the library was
      // pointed elsewhere) indexes itself — no button hunt on first launch.
      if (library.root && !library.dirs.length) rescan();
      const shellPrefs = await invoke("load_shell_prefs");
      layouts[currentMode].focus = !!shellPrefs.focus_mode;
      autoImport = !!shellPrefs.auto_import;
      importDir = shellPrefs.import_dir ?? null;
      const savedPreferences = await invoke("load_preferences").catch(() => ({}));
      aiCullMarkStory = !!savedPreferences.ai_cull_mark_story;
      aiCullExportDesktop = !!savedPreferences.ai_cull_export_desktop;
      aiCullTarget = Number(savedPreferences.ai_cull_target) || 24;
      // The Garden account row: cached username shows instantly, the silent
      // /me re-verify refreshes stats (Swift `refreshIfNeeded`).
      if (shellPrefs.garden_username) {
        gardenAccount = { signed_in: true, username: shellPrefs.garden_username, tier: shellPrefs.garden_tier, notes_count: 0, total_views: 0 };
      }
      invoke("garden_refresh").then((info) => (gardenAccount = info)).catch(() => {});
      on("garden-account-changed", (e) => (gardenAccount = e.payload));
      on("open-file-requested", (e) => openPhoto(e.payload));
      on("dock-reopen-requested", () => switchMode("cull"));
      on("focus-mode-changed", (e) => {
        layouts[currentMode].focus = !!e.payload.enabled;
        saveLayouts();
      });
      on("shell-prefs-changed", (e) => {
        autoImport = !!e.payload.auto_import;
        importDir = e.payload.import_dir ?? null;
        layouts[currentMode].focus = !!e.payload.focus_mode;
        saveLayouts();
      });
      // The scan commits per directory now — refresh the sidebar tree on a
      // slow cadence while it walks, so folders appear as they're indexed.
      let lastTreeRefresh = 0;
      on("index-progress", (e) => {
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
      on("cards-changed", (e) => (cards = e.payload));
      on("card-mounted", (e) => {
        const card = e.payload;
        notify(`card detected · ${card.name} (${card.raw_count})`, 4000);
        if (autoImport && !activity.progress) importCard(card);
      });
      on("card-unmounted", (e) => {
        // Card physically pulled (or ejected): drop any stale eject affordance.
        const dcim = e.payload?.dcim;
        if (ejectableCard && dcim && ejectableCard.dcim === dcim) ejectableCard = null;
      });
      on("import-first-card-requested", async () => {
        const available = await invoke("find_cards");
        cards = available;
        if (available.length) importCard(available[0]);
      });
      on("toggle-auto-import-requested", () => toggleAutoImport());
      on("add-library-folder-requested", () => indexRoot());
      // Settings lives in its own window and shares no memory with this one;
      // a library added or removed there must still reach the folder tree.
      on("libraries-changed", () => refreshDirs());
      // The archive is a NAS mount; it can vanish mid-session or not be up
      // yet at launch. Reveal keeps working from its local cache, but the
      // photographer should know they are looking at what this machine
      // remembers rather than at the archive itself.
      on("source-offline", () => {
        if (sourceOffline) return;
        sourceOffline = true;
        clearInterval(sourceWatch);
        sourceWatch = setInterval(async () => {
          const probe = photoPath || view[sel]?.path;
          if (!probe) return;
          if (await invoke("source_reachable", { path: probe }).catch(() => false)) {
            sourceOffline = false;
            clearInterval(sourceWatch);
          }
        }, 4000);
      });
      on("app-error", (e) => {
        notify(e.payload.message ?? String(e.payload), 5000);
      });
      let lastImportRefresh = 0;
      on("import-progress", async (e) => {
        const payload = e.payload;
        setProgress({ verb: "import", ...payload });
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
            if (!library.curDir) {
              lastImportedFolder = payload.destDir;
              openDir(payload.destDir);
            } else if (library.curDir === payload.destDir) {
              try {
                const rawRows = await invoke("index_frames", { dir: library.curDir, minRating });
                const rows = (Array.isArray(rawRows) ? rawRows : []).filter(
                  (r) => r.name && !r.name.startsWith(".") && !r.name.startsWith("._")
                );
                if (library.curDir === payload.destDir) refreshLoadedFrames(rows);
              } catch (err) {}
            }
          }
        }
      });
      on("import-started", (e) => {
        setProgress({ verb: "import", done: 0, total: 1, current: "Starting..." });
        importedByFolder = new Map();
      });
      on("import-finished", async (e) => {
        setProgress(null);
        notify(`Import complete ✓`, 4000);
        const stats = e.payload;
        if (stats?.folders?.length) {
          const lastFolder = stats.folders[stats.folders.length - 1];
          lastImportedFolder = lastFolder;
          await refreshDirs();
          await openDir(lastFolder);
        }
        // Walk-away AI cull: one folder at a time (not concurrently, so a
        // multi-day card doesn't hammer the vision API in parallel), only
        // for folders that actually received photos this run.
        if ((aiCullMarkStory || aiCullExportDesktop) && stats?.folders?.length) {
          for (const imported of stats.folders) {
            const folderPaths = importedByFolder.get(imported);
            if (folderPaths?.length) await triggerAiCull(imported, folderPaths);
          }
        }
      });
      on("import-failed", (e) => {
        setProgress(null);
        notify(`Import failed: ${e.payload.message}`, 6000);
      });
      // AI cull toasts — same notify() path as import/export
      // rather than a dedicated chip/modal (the flow is walk-away, no review
      // step to build UI for).
      on("cull-started", (e) => {
        cullTaskId = startActivity("cull", `AI Culling · ${e.payload?.total ?? "?"} photos`, e.payload?.total ?? 1);
      });
      on("cull-progress", (e) => {
        const p = e.payload;
        const phaseLabel = p.phase === "cloud" ? "visual analysis" : "local sort";
        // The bottom-center activity indicator already shows this live, no
        // separate toast needed — `progress` itself is still needed as the
        // concurrency guard other handlers check before starting.
        setProgress({ verb: "cull", done: p.done, total: p.total, current: phaseLabel });
        if (cullTaskId) updateActivity(cullTaskId, { current: phaseLabel, done: p.done, total: p.total });
      });
      on("cull-finished", (e) => {
        const stats = e.payload;
        const outcome = stats.exported_to
          ? (stats.marked > 0 ? `added to story, exported → ${stats.exported_to}` : `exported → ${stats.exported_to}`)
          : (stats.marked > 0 ? "added to story" : "kept");
        notify(`AI Culling ✓ ${stats.picked}/${stats.considered} kept · ${outcome}`, 6000);
        if (activity.progress?.verb === "cull") setProgress(null);
        if (cullTaskId) {
          updateActivity(cullTaskId, { done: stats.picked, total: stats.considered, current: outcome, phase: "Complete", status: "completed" });
          releaseActive(cullTaskId);
          cullTaskId = null;
        }
      });
      on("cull-failed", (e) => {
        notify(`AI Culling : ${e.payload.message}`, 6000);
        if (activity.progress?.verb === "cull") setProgress(null);
        if (cullTaskId) {
          updateActivity(cullTaskId, { phase: String(e.payload.message), status: "failed" });
          releaseActive(cullTaskId);
          cullTaskId = null;
        }
      });
      on("export-progress", (e) => {
        setProgress({ verb: "export", ...e.payload });
        if (activity.activeId) {
          updateActivity(activity.activeId, {
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
          setTimeout(() => { setProgress(null); }, 3000);
        }
      });
      on("publish-progress", (e) => {
        setProgress({ verb: e.payload.phase || "publication", ...e.payload });
        if (publishTaskId) {
          updateActivity(publishTaskId, {
            current: e.payload.current || e.payload.phase || "",
            done: e.payload.done,
            total: e.payload.total,
          });
        }
      });
      const pollCards = async () => (cards = await invoke("find_cards"));
      pollCards();
      setInterval(pollCards, 5000);

      if (isTauri) {
        on("dev-panel-ready", () => {
          sendDevStateToPanel();
        });
        on("dev-panel-recipe-updated", (e) => {
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
        on("dev-panel-engine-updated", (e) => {
          applyEngineChange(e.payload.engine);
        });
        on("dev-panel-caption-updated", (e) => {
          caption = e.payload.caption;
          captionEdited();
        });
        on("dev-panel-tags-updated", (e) => {
          tags = e.payload.tags;
          tagsEdited();
        });
        on("dev-panel-export", () => {
          exportCurrent();
        });
        on("dev-panel-export-desktop", () => {
          exportCurrent(""); // "" = the Desktop, regardless of the configured export folder
        });
        on("dev-panel-export-vault", () => {
          exportToDailyNote();
        });
        on("dev-panel-export-daily", () => {
          exportToDailyNote();
        });
        // RESET — back to the engine defaults, like Swift's
        // resetSettings; the sidecar re-saves through the normal edit path.
        on("dev-panel-reset", applyResetRecipe);
        on("dev-panel-export-settings-changed", (e) => {
          exportEdge = e.payload.exportEdge;
          exportBorder = e.payload.exportBorder;
          saveExportPrefs();
        });
        on("dev-panel-photo-scale-changed", (e) => {
          developPhotoPercent = e.payload.photoScale;
        });
        on("dev-panel-choose-export-folder", () => {
          chooseExportFolder();
        });
        on("dev-panel-open-in-editor", (e) => {
          openInEditor(e.payload.appPath);
        });
        on("dev-panel-switch-mode", (e) => {
          switchMode(e.payload.mode);
        });
        // The dev panel is a separate OS window, so it steals keyboard focus:
        // shortcuts pressed there never reached the main window (g, arrows, …
        // did nothing until you clicked back). It forwards them here and we
        // replay them through the same onKey with a synthetic event.
        on("dev-panel-key", (e) => {
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
        on("dev-panel-zoom", (e) => {
          cycleZoom(e.payload?.reverse);
        });
        on("dev-panel-close", () => {
          layouts.dev.devPanel = false;
          saveLayouts();
        });
        // The detached panel's own "dock" button asks to come back inline —
        // dropping the flag hides the external window (reactive effect below)
        // and the docked <DevelopPanel> takes over.
        on("dev-panel-dock-requested", () => {
          layouts.dev.detached = false;
          saveLayouts();
        });
        on("settings-panel-ready", () => sendSettingsToPanel());
        on("settings-panel-choose-folder", (e) => {
          choosePreferenceFolder(/** @type {any} */ (e.payload)?.key);
        });
        on("settings-panel-save", (e) => {
          saveSettingsFromPanel(/** @type {any} */ (e.payload)?.preferences);
        });
        on("open-settings-requested", openSettings);
        on("toggle-dev-panel-requested", toggleDevPanel);
        on("toggle-preset-panel-requested", togglePresetPanel);
        on("toggle-lut-panel-requested", toggleLutPanel);
        /** @type {Recipe | null} */ let savedRecipeBeforeHover = null;
        /** @type {string | null} */ let savedEngineBeforeHover = null;

        on("preset-preview", (e) => {
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

        on("dev-panel-toggle-clipping", (e) => {
          const payload = e.payload || {};
          if (typeof payload.showClipping === "boolean") {
            showClipping = payload.showClipping;
          } else {
            showClipping = !showClipping;
          }
          sendDevStateToPanel();
        });
        on("dev-panel-toggle-caption", (e) => {
          const payload = e.payload || {};
          if (typeof payload.showCaption === "boolean") {
            showCaption = payload.showCaption;
          } else {
            showCaption = !showCaption;
          }
          sendDevStateToPanel();
        });

        // The Preset palette asks the main window to apply a saved recipe. The
        // main window owns the selection, so it resolves the target set here:
        // "selected" (modifier held) → all selected; "auto" → all selected when
        // several are, else just the current photo.
        on("preset-apply", (e) => {
          savedRecipeBeforeHover = null;
          savedEngineBeforeHover = null;
          const payload = e.payload || {};
          const presetRecipe = payload.recipe;
          if (!presetRecipe) return;
          const selected = selectedFrames(view);
          const current = view[sel] ? [view[sel]] : selected;
          const targets =
            payload.scope === "selected"
              ? (selected.length ? selected : current)
              : (selected.length > 1 ? selected : current);
          applyRecipeToFrames(presetRecipe, targets);
        });
        on("toggle-render-queue-requested", () => setQueueOpen());
        on("menu-export-requested", () => {
          if (currentMode === "dev" && photoPath && recipe) exportCurrent();
          else exportSelection();
        });
        on("menu-reset-develop-requested", async () => {
          if (recipe) {
            recipe = await invoke("default_recipe");
            edited();
          }
        });
        on("menu-clear-develop-requested", clearDevelopment);
        on("menu-enable-develop-requested", () => {
          if (currentMode !== "dev") switchMode("dev");
          else edited(false);
        });
        on("menu-publish-requested", () => {
          if (preferences.obsidian_enabled && view[sel]) developFromMenu(view[sel].path, true);
        });
      }

      // A deep-link/"open with" target overrides whatever was already opened
      // above; otherwise there's nothing left to do here — the boot sequence
      // already landed on the right directory (or Grid) once.
      const auto = await invoke("autoload_path") || await invoke("take_open_file");
      if (auto) {
        if (auto.endsWith("/") || !auto.includes(".")) openFolder(auto);
        else openPhoto(auto);
      }
    })();
    return () => {
      for (const pending of wired) pending.then((stop) => stop()).catch(() => {});
    };
  });

  // ---- modes, and the photo surface ----------------------------------------------
  // Switching between grid and Develop, the zoom cycle, the pan gesture, and
  // focus mode with the pointer-presence effect that feeds it.

  function saveLayouts() {
    session.setModeLayouts(layouts);
  }

  /**
   * @param {"cull" | "dev"} to
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  async function switchMode(to, { openDevPanel = true } = {}) {
    closePhotoMenu();
    if (to !== "dev") {
      zoomMode = "frame"; // always re-enter develop framed
      scheduleWorkingRelease();
    }
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
    session.setLastMode(to);
    // Only through the open folder's session: a folder nothing has been read
    // for is a folder nothing may be written for.
    if (folderSession?.dir === library.dir) folderSession.saveMode(to);

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

  /**
   * Flip the Editorial filter — same guards the old "story" mode had (needs a
   * real folder; Apple Photos albums are read-only, nothing to preview).
   * @param {boolean} [value] force a value instead of toggling
   */
  function togglePreviewFilter(value) {
    const next = value ?? !previewFilter;
    if (next) {
      if (applePhotosActive) {
        hold("Editorial needs a filesystem folder. You can edit and export Apple Photos directly.");
        return;
      }
      if (!library.curDir && !library.folder) return;
    }
    previewFilter = next;
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
    // "actual" (1:1 pixels) always pans on drag; "frame" only once the Photo
    // Size slider has pushed the photo past 100% and it actually overflows —
    // below that, the same drag is the loupe's gesture instead (DevelopView).
    const canPan = zoomMode === "actual" || (zoomMode === "frame" && developPhotoPercent > 100);
    if (!canPan || e.button !== 0) return;
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
    const onFocus = () => setPresence(true);
    const onBlur = () => {
      if (!pointerInside) setPresence(false);
    };
    const onPointerEnter = () => setPresence(true);
    const onPointerLeave = () => {
      pointerInside = false;
      if (!document.hasFocus()) setPresence(false);
    };
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

  // ---- the detached panels -------------------------------------------------------
  // Develop and its palettes as separate OS windows: what they are told, where
  // they are put (see $lib/palettePlacement.js), and when they are synced.
  function sendDevStateToPanel() {
    if (isTauri && currentMode === "dev") {
      emit("main-dev-state", {
        photoPath: photoPath,
        picked: picked,
        rating: library.frames.find((f) => f.path === photoPath)?.rating ?? 0,
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
        tags: tags,
        showClipping: showClipping,
        showCaption: showCaption,
        photoScale: developPhotoPercent,
        // Typed arrays don't survive Tauri's JSON emit as themselves — plain
        // arrays round-trip fine and index identically in Histogram.svelte.
        histogram: histogram
          ? { r: Array.from(histogram.r), g: Array.from(histogram.g), b: Array.from(histogram.b), luma: Array.from(histogram.luma) }
          : null,
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
    if (label === "develop-panel") return picked ? `${picked} — Darkroom` : "Darkroom";
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
    // Measuring only. The geometry that decides right / left / other screen
    // lives in $lib/palettePlacement.js, where its awkward cases are tested
    // instead of being reproduced by dragging a window around.
    try {
      const main = getCurrentWindow();
      const factor = await main.scaleFactor();
      const outer = await main.outerPosition();
      const size = await main.outerSize();
      const mainMon = await currentMonitor();
      if (!mainMon?.position || !mainMon?.size) return null;

      const onSameMonitor = (/** @type {{ position: { x: number, y: number } }} */ m) =>
        m.position.x === mainMon.position.x && m.position.y === mainMon.position.y;
      const others = (await availableMonitors())
        .filter((m) => !onSameMonitor(m))
        .map((m) => ({ x: m.position.x / factor, y: m.position.y / factor }));

      return placePalette({
        main: { x: outer.x / factor, y: outer.y / factor, width: size.width / factor },
        monitor: { x: mainMon.position.x / factor, width: mainMon.size.width / factor },
        otherMonitors: others,
        index,
        panelWidth,
      });
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
      // Docked is the default now — only actually open/show the OS window
      // when the panel has been explicitly detached.
      if (currentMode === "dev" && layouts.dev[spec.flag] && layouts.dev.detached && !spaceLook) {
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
          await win.hide();
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
      const trigger = [photoPath, picked, recipe, developEngine, renderMs, status, installedEditors, exportEdge, exportBorder, films, papers, luts, caption, tags, currentRating, histogram, developPhotoPercent];
      sendDevStateToPanel();
    }
  });

  $effect(() => {
    if (isTauri) {
      // Reactively sync every palette when mode, any palette flag, or quick-look
      // toggles. spaceLook suppresses all palettes so quick-look stays clean.
      const trigger = [currentMode, layouts.dev.devPanel, layouts.dev.detached, spaceLook];
      syncPalettes();
    }
  });

  // ---- window chrome -------------------------------------------------------------
  function toggleSidebar() {
    closeSidebarPeek();
    layouts[currentMode].sidebar = !layouts[currentMode].sidebar;
    saveLayouts();
  }

  function toggleDevPanel() {
    // The panel's visibility is always stored under layouts.dev, regardless
    // of which mode we're currently in (layouts.cull/story.devPanel are
    // unused) — every reader of the flag reads layouts.dev.devPanel.
    // ⇧D during a quick look just reveals the panel the look was suppressing,
    // rather than toggling the stored preference off.
    if (spaceLook && layouts.dev.devPanel) {
      spaceLook = false;
      if (currentMode !== "dev") switchMode("dev", { openDevPanel: false });
      return;
    }
    if (currentMode !== "dev") {
      // ⇧D is a deliberate develop entry too — jump in with the panel open.
      switchMode("dev", { openDevPanel: true });
      return;
    }
    spaceLook = false;
    layouts.dev.devPanel = !layouts.dev.devPanel;
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
      notify(`appearance: ${e}`, 5000);
    }
  }

  async function hideWindow() {
    await invoke("hide_contact_sheet");
  }

  // ---- refreshing the library and its stories ------------------------------------

  // `light` skips the story-dot probe — one fs read per folder, too heavy to
  // repeat mid-scan over the NFS mount.
  /**
   * The version token for a photo's preview: the `.preview.jpg` mtime, read
   * back from disk after a render settles.
   *
   * This used to be `Date.now()`, which busts the webview's own cache fine
   * but can never match a file — so nothing on disk could be addressed by it.
   * The local render cache keys on this token, so it has to be the truth.
   * @param {string} path
   */
  async function freshPreviewVersion(path) {
    try {
      const [v] = /** @type {number[]} */ (await invoke("preview_versions", { paths: [path] }));
      return v || Date.now();
    } catch {
      return Date.now();
    }
  }

  async function refreshDirs(light = false) {
    try {
      const [r, d] = await invoke("index_dirs");
      setRoots(r); // index_dirs now returns the full root set
      setDirs(d);
      if (!light) refreshStoryDirs();
      return true;
    } catch (error) {
      /** @type {RevealWindow} */ (window).__log?.(`folder tree unavailable: ${error}`);
      return false;
    }
  }

  // The sidebar's red "this day has a story" dots.
  async function refreshStoryDirs() {
    if (!isTauri || !library.dirs.length) return;
    try {
      storyDirs = new Set(await invoke("story_dirs", { dirs: library.dirs.map((d) => d.dir) }));
    } catch (e) {}
    // Refresh the story-notes lists too — pin/story changes flow through here.
    await refreshStoryNotes();
  }

  // Load every folder's story note metadata and split into the pinned list
  // (sorted by pinned-at descending — newest/newest-pinned at top) and the
  // recent list (mtime desc, top 12). Pinned notes may also appear in recent.
  async function refreshStoryNotes() {
    if (!isTauri || !library.dirs.length) {
      pinnedStories = [];
      recentStories = [];
      return;
    }
    /** @type {any[]} */
    let all = [];
    try {
      all = await invoke("list_story_notes", { dirs: library.dirs.map((d) => d.dir) });
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


  // ---- what the grid shows -------------------------------------------------------
  // The one derivation every other surface reads, and the selection positions
  // resolved against it, plus the preferences that shape it.
  /**
   * Masonry lays out with CSS `column-count`, and CSS columns cannot be
   * windowed — you cannot know which items are in view — so unlike the
   * uniform grid it puts EVERY photo in the DOM at once. That is the whole
   * reason for the ceiling: not the layout, the absence of virtualisation.
   */
  const MASONRY_LIMIT = 500;
  const masonryTooBig = $derived(library.frames.length > MASONRY_LIMIT);

  function saveGridPrefs() {
    session.setGridPrefs({ cols, marginScale, cellAspect, fillCells, sortDesc });
  }

  // What the grid actually shows: the backend rows, optionally narrowed to
  // the story set (Collection Rapide), in the chosen name order.
  const view = $derived.by(() => {
    let rows = library.frames;
    if (applePhotosActive && minRating) rows = rows.filter((frame) => frame.rating >= minRating);
    if (filterStory) rows = rows.filter((f) => storySet.has(stem(f.name)));
    // Ascending = the backend's ORDER BY capture_at, name. Descending flips
    // on the same key — capture date first, filename as the tiebreak.
    if (sortDesc && !applePhotosActive) {
      rows = [...rows].sort(
        (a, b) => Number(b.capture_at || 0) - Number(a.capture_at || 0) || b.name.localeCompare(a.name),
      );
    }
    return rows;
  });

  /**
   * Where the focus and the range anchor sit in the CURRENT view.
   *
   * Derived, never assigned: see src/lib/selection.svelte.js for why the
   * position is computed from the photo rather than stored beside it.
   */
  const sel = $derived(positionIn(view));
  const selectionAnchor = $derived(anchorIn(view));


  /**
   * @param {number} index
   * @param {MouseEvent} event
   */
  function openPhotoMenu(index, event) {
    event.preventDefault();
    const frame = view[index];
    if (!frame) return;
    if (!selection.paths.has(frame.path)) selectOnly(view, index);
    focusAt(view, index);
    photoMenu = {
      frame,
      x: event.clientX,
      y: event.clientY,
    };
  }

  function closePhotoMenu() {
    photoMenu = null;
  }

  // ---- the settings window, and the Garden account -------------------------------

  // Settings is always its own OS window (Francis: it should feel like any
  // other app's Settings, never a dialog over the main one) — same
  // WebviewWindow pattern as the Develop palettes, minus the position
  // cascade (it isn't tied to Develop mode) and minus continuous sync (one
  // snapshot out, one save or folder-pick round trip back).
  function sendSettingsToPanel() {
    if (isTauri) emit("main-settings-state", { preferences }).catch(() => {});
  }

  async function openSettings() {
    if (isTauri) {
      const saved = await invoke("load_preferences");
      preferences = { ...preferences, ...saved, export_folder: saved.export_folder ?? exportFolder };
    }
    if (!isTauri) return;
    let win = await WebviewWindow.getByLabel("settings-panel");
    if (win) {
      await win.show();
      await win.setFocus();
      sendSettingsToPanel();
    } else {
      win = new WebviewWindow("settings-panel", {
        url: "/settings-panel",
        title: "Settings",
        width: 640,
        height: 480,
        resizable: true,
        titleBarStyle: "overlay",
        hiddenTitle: true,
      });
      win.once("tauri://created", () => setTimeout(sendSettingsToPanel, 300));
    }
  }

  /** @param {typeof preferences} newPreferences */
  async function saveSettingsFromPanel(newPreferences) {
    preferences = newPreferences;
    if (isTauri) await invoke("save_preferences", { preferences });
    exportFolder = preferences.export_folder ?? "";
    saveExportPrefs();
    aiCullMarkStory = !!preferences.ai_cull_mark_story;
    aiCullExportDesktop = !!preferences.ai_cull_export_desktop;
    aiCullTarget = Number(preferences.ai_cull_target) || 24;
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
    if (path) {
      preferences = { ...preferences, [key]: path };
      emit("settings-panel-folder-chosen", { key, path }).catch(() => {});
    }
  }

  // ---- the main window, and fullscreen -------------------------------------------
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
        "button, input, select, textarea, a, nav, dialog, [role='dialog'], [role='button'], [role='menu'], [contenteditable='true'], [draggable='true'], .cell, .photo-cell, .roll-cell, .frame, .gap, .composer, .surface, .roll, [role='listitem'], .sidebar-peek, .photo-mat",
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
      const frame = library.frames.find((item) => item.path === path);
      const immediate =
        currentMode === "dev" && photoPath === path && imgUrl
          ? imgUrl
          : previewUrl(path, frame?.previewVersion ?? 0);
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
      prepareFullscreenFrame(frame.path);
      if (isTauri) {
        const panel = await WebviewWindow.getByLabel("develop-panel");
        if (panel) await panel.hide();
        // Lightroom-style borderless fullscreen (fill the display in place, no
        // Spaces animation) — NOT the macOS native setFullscreen.
        await invoke("set_simple_fullscreen", { enabled: true });
        try {
          await getCurrentWindow().setFocus();
        } catch {}
      }
      fullscreen = true;
      window.focus();
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
        try {
          await getCurrentWindow().setFocus();
        } catch {}
        if (currentMode === "dev" && layouts.dev.devPanel) await syncDevPanelWindow();
      }
      window.focus();
  }

  function toggleFullscreen() {
    if (fullscreen) exitFullscreen();
    else enterFullscreen();
  }

  /** @param {string} path */
  // ---- opening a photo somewhere else --------------------------------------------
  // Finder, Preview, an external editor, or Develop from the menu bar.
  async function revealPhotoInFinder(path) {
    closePhotoMenu();
    try {
      await invoke("reveal_in_finder", { path });
    } catch (error) {
      hold(`Could not reveal photo: ${error}`);
    }
  }

  /** @param {string} path */
  async function openPhotoPreview(path) {
    closePhotoMenu();
    if (path.startsWith("apple-photos://")) {
      await openPhoto(path, { openDevPanel: false });
      return;
    }
    try {
      await invoke("open_path", { path });
    } catch (error) {
      hold(`Could not open photo: ${error}`);
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
      hold(`Could not open editor: ${error}`);
    }
  }

  /**
   * @param {string} path
   * @param {boolean} toVault
   */
  async function developFromMenu(path, toVault) {
    closePhotoMenu();
    /** @type {string | null} */ let devJobId = null;
    try {
      const sidecar = await invoke("load_sidecar", { path });
      if (!sidecar?.engine_settings) {
        notify("No development engine is active for this photo", 3000);
        return;
      }
      if (toVault) {
        if (!preferences.obsidian_enabled) {
          notify("Obsidian integration is disabled in settings", 3000);
          return;
        }
        await exportSelectionToDailyNote(path);
        return;
      }
      setProgress({ verb: "Developing", done: 0, total: 1, current: path.split("/").pop() });
      devJobId = startActivity("develop", `Developing ${path.split("/").pop()}`, 1);
      await invoke("export_photo", {
        path,
        recipe: sidecar.engine_settings,
        destDir: exportFolder,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      patchProgress({ done: 1 });
      updateActivity(devJobId, { done: 1, phase: "Complete", status: "completed" });
    } catch (error) {
      hold(`Development failed: ${error}`);
      if (devJobId) updateActivity(devJobId, { phase: String(error), status: "failed" });
    } finally {
      releaseActive(devJobId);
      if (activity.progress) {
        setTimeout(() => {
          setProgress(null);
        }, 1200);
      }
    }
  }

  // ---- the catalogue's roots -----------------------------------------------------
  // Adding, listing, forgetting and rescanning the folders the index is built
  // from — the library is a SET of roots, not one.
  async function indexRoot() {
    const path = await invoke("pick_folder");
    if (!path) return;
    scanning = true;
    try {
      await invoke("scan_root", { path });
      await refreshDirs();
      if (library.dirs.length) openDir(library.dirs[library.dirs.length - 1].dir);
    } finally {
      scanning = false;
    }
  }

  /** Every registered library with its frame count and whether it's mounted. */
  async function listLibraries() {
    try {
      return await invoke("catalog_roots");
    } catch (error) {
      /** @type {RevealWindow} */ (window).__log?.(`libraries unavailable: ${error}`);
      return [];
    }
  }

  /**
   * Forget a library. Only the catalogue entry goes — the photos on disk are
   * never touched — but the ratings, captions and story marks the catalogue
   * holds for those files go with it, which is why every caller confirms
   * first.
   * @param {string} path
   */
  async function removeLibrary(path) {
    const pruned = await invoke("remove_catalog_root", { path });
    // If we were looking inside the library that just left, step out of it
    // rather than leaving the grid pointed at a folder the index forgot.
    // `view` is derived from the frames of `curDir`, so clearing the folder
    // empties the grid on its own.
    if (library.curDir === path || library.curDir?.startsWith(`${path}/`)) {
      leaveFolder();
    }
    await refreshDirs();
    notify(`Library removed · ${Number(pruned).toLocaleString("en-CA")} photos forgotten`, 4000);
    return pruned;
  }

  /** @param {string} path */
  async function rescanLibrary(path) {
    scanning = true;
    try {
      await invoke("scan_root", { path });
      await refreshDirs();
    } finally {
      scanning = false;
    }
  }

  async function rescan() {
    if (applePhotosActive) {
      await refreshApplePhotos();
      return;
    }
    if (!library.roots.length) return;
    scanning = true;
    try {
      for (const r of library.roots) await invoke("scan_root", { path: r });
      await refreshDirs();
      if (library.curDir) openDir(library.curDir);
      else if (library.dirs.length) openDir(library.dirs[library.dirs.length - 1].dir);
    } finally {
      scanning = false;
    }
  }

  /** @param {string} path */
  async function rescanDir(path) {
    if (!path || scanning) return;
    scanning = true;
    hold(`Reindexing ${path.split("/").pop()}…`);
    try {
      await invoke(library.roots.includes(path) ? "scan_root" : "scan_folder", { path });
      await refreshDirs();
      if (library.curDir?.startsWith(path)) await openDir(library.curDir);
      hold(`Reindexed ${path.split("/").pop()}`);
    } catch (error) {
      hold(`Could not reindex folder: ${error}`);
    } finally {
      scanning = false;
      setTimeout(() => {
        if (activity.message.startsWith("Reindexed ")) dismiss();
      }, 2200);
    }
  }

  /** @param {string} path */
  async function revealDir(path) {
    try {
      await invoke("open_path", { path });
    } catch (error) {
      hold(`Could not open folder: ${error}`);
    }
  }

  async function toggleAutoImport() {
    const prefs = await invoke("toggle_auto_import");
    autoImport = !!prefs.auto_import;
    notify(autoImport ? "auto-import enabled" : "auto-import disabled", 2500);
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
      notify(`Import → ${name}`, 2500);
    } catch (error) {
      hold(`Could not set import folder: ${error}`);
    }
  }

  // ---- dragging a photo onto a folder to move it there ---------------------------
  // The dragged frame carries its path(s) on the drag session; a folder row in
  // the sidebar reads them on drop and calls movePhotos. Dragging a photo that's
  // part of the current selection moves the whole selection.
  /**
   * @param {string} path
   * @param {DragEvent} event
   */
  function onPhotoDragStart(path, event) {
    if (path.startsWith("apple-photos://")) {
      event.preventDefault();
      hold("Export Apple Photos before moving them to a folder.");
      return;
    }
    if (!event.dataTransfer) return;
    const paths =
      selection.paths.has(path) && selection.paths.size > 1 ? [...selection.paths] : [path];
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
      notify("already in this folder", 2500);
      return;
    }
    const srcDirs = new Set(toMove.map(parentOf));
    setProgress({ verb: "move", done: 0, total: toMove.length, current: "" });
    const jobId = startActivity("move", `Moving ${toMove.length} photo(s)`, toMove.length);
    let moved = 0;
    const errors = [];
    for (const p of toMove) {
      try {
        await invoke("move_photo", { path: p, destDir });
        moved += 1;
        setProgress({ verb: "move", done: moved, total: toMove.length, current: p.split("/").pop() });
        updateActivity(jobId, { done: moved, current: p.split("/").pop() });
      } catch (e) {
        errors.push(`${p.split("/").pop()} : ${e}`);
      }
    }
    // Reconcile the index — the destination gains frames, each source loses them.
    // In a `finally` because reopening the folder can throw, and it used to be
    // the last thing standing between a failure and a progress bar left
    // spinning for the rest of the session: nine of the app's eleven long jobs
    // already cleaned up this way, this one did not.
    const destName = destDir.split("/").pop();
    try {
      try {
        await invoke("scan_folder", { path: destDir });
        for (const d of srcDirs) await invoke("scan_folder", { path: d });
      } catch (_) {}
      await refreshDirs();
      if (library.curDir) await openDir(library.curDir);
      clearSelection();
    } finally {
      setProgress(null);
      hold(
        errors.length
          ? `${moved} moved · ${errors.length} failed`
          : `${moved} photo${moved > 1 ? "s" : ""} moved → ${destName}`,
      );
      updateActivity(jobId, {
        current: destName,
        phase: errors.length ? `${errors.length} failed` : "Complete",
        status: errors.length ? "failed" : "completed",
      });
      releaseActive(jobId);
    }
    if (errors.length) console.warn("move errors:", errors);
  }

  /** @param {string} destDir */
  function moveSelectedPhotosToDir(destDir) {
    const paths = [...selection.paths];
    if (!paths.length && view[sel]) paths.push(view[sel].path);
    if (paths.length) movePhotos(paths, destDir);
  }

  // ---- renaming, creating and moving folders -------------------------------------
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
    if (!library.root) return;
    try {
      await invoke("scan_root", { path: library.root });
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
      if (library.curDir === path) {
        await openDir(newPath);
      } else if (library.curDir && library.curDir.startsWith(path + "/")) {
        await openDir(newPath + library.curDir.slice(path.length));
      }
      notify(`Renamed → ${newName}`, 3000);
    } catch (e) {
      notify(`Rename failed: ${e}`, 3000);
    }
  }

  /**
   * @param {string} parentDir
   * @param {string} name
   */
  async function createFolder(parentDir, name) {
    try {
      const abs = await invoke("create_dir", { parentDir, name });
      notify(`Folder created: ${name}`, 2500);
      return abs;
    } catch (e) {
      notify(`Creation failed: ${e}`, 4000);
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
      if (library.curDir === path) {
        await openDir(newPath);
      } else if (library.curDir && library.curDir.startsWith(path + "/")) {
        await openDir(newPath + library.curDir.slice(path.length));
      }
      notify(`${name} moved`, 3000);
    } catch (e) {
      notify(`Move failed: ${e}`, 3000);
    }
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
  /**
   * @param {string} dir
   * @param {boolean} [restoreMode] Apple Photos only — authorize interactively (true) or stay silent (false, startup)
   * @param {boolean} [restoreSession] re-enter Develop on whichever photo was open last time, instead of always landing in Grid
   */
  async function openDir(dir, restoreMode = true, restoreSession = false) {
    if (dir?.startsWith(APPLE_PHOTOS_ROOT)) {
      await openApplePhotos(dir.slice(APPLE_PHOTOS_ROOT.length), { authorize: restoreMode });
      return;
    }
    leaveApplePhotos();
    // NOTE: the index stores dirs in the canonical firmlink form
    // (/System/Volumes/Data/mnt/…) — pass paths through verbatim; any
    // "normalization" to the short alias breaks the exact-match query.
    //
    // The token is what makes the commits below safe. This used to compare
    // `request !== applePhotosRequest` — a counter belonging to the Apple
    // Photos path, captured here and never incremented by openDir, so
    // between two filesystem folders it compared N against N and let a
    // stale load through.
    const open = beginOpen({ curDir: dir });
    session.setLastDirectory(dir);
    // Folder switch always starts with the full contact sheet.
    minRating = 0;
    filterStory = false;
    previewFilter = false;
    debug = "invoke…";
    /** @type {RevealWindow} */ (window).__log?.(`openDir start dir=${dir} minRating=${JSON.stringify(minRating)}`);
    try {
      const rawRows = await invoke("index_frames", { dir, minRating });
      const rows = (Array.isArray(rawRows) ? rawRows : []).filter(r => r.name && !r.name.startsWith('.') && !r.name.startsWith('._'));
      debug = `received ${rows.length}`;
      if (!open.commit(rows)) return; // overtaken by another folder
      if (masonryTooBig && layout === "masonry") {
        layout = "uniform"; // Fallback to virtualized grid to prevent memory/CPU explosion
      }
      withPreviewVersions(rows).then((updated) => {
        // Fresh array ref: `updated` is the same array we mutated in place, and
        // a raw $state only reacts to an identity change.
        if (Array.isArray(updated)) open.replace(updated);
      });
    } catch (e) {
      debug = `failed: ${e}`;
      /** @type {RevealWindow} */ (window).__log?.(`openDir failed: ${e}`);
    }
    // Restore the viewport BEFORE the selection, and not on a timer.
    //
    // PhotoGrid reads this prop through two effects as soon as it mounts. One
    // keeps the selected cell visible and bails out only `if (initial &&
    // scrollTop > 0)` — a guard that is exactly right and was being evaluated
    // too early. Restoring 50ms later meant the grid mounted believing you
    // were at the top, scrolled to cell 0 to "keep it visible", and wrote
    // that 0 back through onScroll, over the position we had just read.
    //
    // So the stored offset was saved correctly every time and destroyed on
    // the way back in. A provisional value plus an observer that writes what
    // it observes makes the provisional value permanent.
    folderSession = openFolderSession(dir);
    currentScrollTop = folderSession.scroll || scrollOffsets[dir] || 0;
    focusAt(view, 0);
    selectOnly(view, 0);
    let restoredToDevelop = false;
    if (restoreSession) {
      const savedPhoto = session.lastPhoto();
      // `view`, not `frames`: `sel` indexes the VIEW, which filters by rating
      // or story and reverses under `sortDesc`. Looking the photo up in
      // `frames` produced an index that was valid for the wrong list — with
      // descending sort, exactly the mirror position.
      //
      // The damage was not the wrong highlight. `openPhoto` switches to
      // Develop, and `switchMode` then sees `view[sel]` disagreeing with
      // `photoPath` and opens `view[sel]` to "fix" it — so the app restored
      // photo A and then immediately opened its mirror B, parked B, and saved
      // B as the last photo. The next launch mirrored back to A. Measured on
      // a 61-photo folder: positions 16 and 46, alternating every relaunch,
      // each cold start paying 3.4s of NAS read the parked frame existed to
      // avoid.
      const savedIdx = savedPhoto ? view.findIndex((f) => f.path === savedPhoto) : -1;
      if (savedPhoto && folderSession.mode === "dev" && savedIdx !== -1) {
        focusAt(view, savedIdx);
        selectOnly(view, savedIdx);
        await openPhoto(savedPhoto, { openDevPanel: layouts.dev.devPanel });
        restoredToDevelop = true;
      }
    }
    if (!restoredToDevelop) await switchMode("cull");
    refreshStory();
    setLoading(false); // frames have settled — re-enable the empty-state for genuinely empty folders
  }


  // ---- stories -------------------------------------------------------------------
  /** @param {string} name */
  const stem = (name) => name.replace(/\.[^.]+$/, "");

  async function loadCatalogNote() {
    if (!library.root) return;
    catalogContent = await invoke("load_catalog_note", { root: library.root });
  }

  async function saveCatalogNote() {
    if (!library.root) return;
    await invoke("save_catalog_note", { root: library.root, content: catalogContent });
  }

  $effect(() => {
    if (library.root) {
      loadCatalogNote();
    }
  });

  async function loadStory() {
    const d = library.dir;
    if (!d) return;
    storyContent = await invoke("load_story_note", { dir: d });
  }

  // The WYSIWYG composer's persist hook: write its serialized note to disk and
  // refresh the story set (film-roll markers + sidebar dots). Deliberately does
  // NOT touch `storyContent` — that stays the last real load, so the composer
  // (which owns the blocks this session) never re-inits from its own output.
  /** @param {string} content */
  async function saveStoryContent(content) {
    const d = library.dir;
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
      notify(`Failed to save the story: ${e}`, 8000);
      return;
    }
    storySet = new Set(await invoke("story_stems", { dir: d }));
    refreshStoryDirs();
  }

  // Grid always shows filename order; the story has its own, independent
  // order (Editorial drag-and-drop). A paragraph's *position in Grid* has to
  // come from somewhere else — so it anchors to whichever story photo it
  // trails in the FILE, and renders after THAT photo's row in Grid. Walking
  // the file once gives every paragraph's anchor; saveGridProse (below)
  // finds the anchor for a NEW paragraph the same way, so what you just
  // typed lands exactly where this map will look for it on next render.
  const storyBlocks = $derived.by(() => parseStory(storyContent).blocks);
  const gridProseByRow = $derived.by(() => {
    /** @type {Map<number, {id: string, text: string}[]>} */
    const map = new Map();
    const rowOf = new Map(view.map((f, i) => [stem(f.name), Math.floor(i / cols)]));
    let anchorRow = -1; // -1 = no story photo seen yet → renders above row 0
    for (const b of storyBlocks) {
      if (b.isPhoto) {
        const row = rowOf.get(b.stem);
        if (row !== undefined) anchorRow = row;
      } else if (b.text.trim()) {
        // A paragraph's own `<!--grid-anchor:STEM-->` (any grid photo, in the
        // story or not) wins over the sequential last-embedded-photo-seen
        // fallback — that fallback only still fires for older notes saved
        // before paragraphs recorded their own anchor.
        const explicitRow = b.stem ? rowOf.get(b.stem) : undefined;
        const row = explicitRow !== undefined ? explicitRow : anchorRow;
        const list = map.get(row) ?? [];
        list.push({ id: b.id, text: b.text });
        map.set(row, list);
      }
    }
    return map;
  });

  /**
   * Grid's hover "+" between rows (or clicking an existing paragraph shown
   * there) — writes straight to the story note, no mode switch.
   * @param {number} row grid row this came from (-1 = before the first photo)
   * @param {string} text
   * @param {string} [blockId] editing/deleting an existing paragraph instead of adding one
   */
  async function saveGridProse(row, text, blockId) {
    if (!library.dir) return;
    const trimmed = (text ?? "").trim();
    const { frontmatter, blocks } = parseStory(storyContent);

    if (blockId) {
      const i = blocks.findIndex((b) => b.id === blockId);
      if (i === -1) return;
      if (!trimmed) blocks.splice(i, 1); // cleared text = remove the paragraph
      else blocks[i] = { ...blocks[i], text: trimmed };
    } else {
      if (!trimmed) return;
      // Two different anchors: where it DISPLAYS in Grid (any photo at or
      // before the hovered row, in the story or not — recorded on the block
      // itself so gridProseByRow can place it there) vs. where it lands IN
      // THE FILE (has to sit after an actually-embedded photo, since that's
      // the only kind of position the markdown format has).
      let displayStem = null;
      let fileAnchorStem = null;
      for (let i = 0; i < view.length; i++) {
        if (Math.floor(i / cols) > row) break;
        const s = stem(view[i].name);
        displayStem = s;
        if (storySet.has(s)) fileAnchorStem = s;
      }
      const anchorIdx = fileAnchorStem ? blocks.findIndex((b) => b.isPhoto && b.stem === fileAnchorStem) : -1;
      blocks.splice(anchorIdx + 1, 0, {
        id: `blk-grid-${Date.now()}`,
        isPhoto: false,
        stem: displayStem ?? "",
        text: trimmed,
        rowBreak: true,
      });
    }

    const next = serializeStory(frontmatter, blocks);
    await saveStoryContent(next);
    storyContent = next;
  }

  async function refreshStory() {
    const d = library.dir;
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
      notify("Opened successfully ✓", 2000);
    } catch (e) {
      notify(`erreur : ${e}`, 5000);
    }
  }

  function toggleLayout() {
    if (layout === "uniform" && masonryTooBig) {
      notify(`Masonry draws every photo at once — ${library.frames.length} is past the limit of ${MASONRY_LIMIT}`, 4000);
      return;
    }
    layout = layout === "uniform" ? "masonry" : "uniform";
    session.setGridLayout(layout);
  }

  /**
   * @param {number | undefined} v
   * @param {number} min
   * @param {number} max
   */
  const pct = (v, min, max) => `${((Number(v ?? 0) - min) / (max - min)) * 100}%`;

  /** @param {string} path */
  async function toggleStoryWithPath(path) {
    if (path.startsWith("apple-photos://")) {
      hold("Apple Photos albums are read-only. Use ratings to select photos, then export.");
      return;
    }
    const d = library.dir;
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
    const noteDir = notePath.substring(0, notePath.lastIndexOf("/"));
    try {
      await invoke("story_set_pinned", { dir: noteDir, pinned, pinnedAt });
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
      const noteDir = reordered[i].notePath.substring(
        0,
        reordered[i].notePath.lastIndexOf("/"),
      );
      try {
        await invoke("story_set_pinned", { dir: noteDir, pinned: true, pinnedAt });
      } catch (e) {}
    }
    await refreshStoryNotes();
  }

  async function publishStory() {
    const d = library.dir;
    if (!d || !storySet.size || !gardenAccount?.signed_in || activity.anyRunning) return;
    liveUrl = null;
    setProgress({ verb: "publication", done: 0, total: storySet.size, current: "" });
    publishTaskId = startActivity("publish", `Publier l'histoire · ${storySet.size} photos`, storySet.size);
    try {
      liveUrl = await invoke("publish_story", { dir: d, dryRun: false });
      notify("published ✓", 2000);
      updateActivity(publishTaskId, { done: storySet.size, phase: "Complete", status: "completed" });
    } catch (e) {
      notify(`erreur : ${e}`, 5000);
      updateActivity(publishTaskId, { phase: String(e), status: "failed" });
    } finally {
      setProgress(null);
      releaseActive(publishTaskId);
      publishTaskId = null;
    }
  }

  async function exportLocalStory() {
    const d = library.dir;
    if (!d || !storySet.size || activity.anyRunning) return;
    const dest = await invoke("pick_folder");
    if (!dest) return;
    liveUrl = null;
    setProgress({ verb: "export", done: 0, total: storySet.size, current: "" });
    const jobId = startActivity("export", `Exporter l'histoire · ${storySet.size} photos`, storySet.size);
    try {
      await invoke("export_local_story", {
        dir: d,
        dest: dest,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0
      });
      notify("exported ✓", 2000);
      updateActivity(jobId, { done: storySet.size, current: dest, phase: "Complete", status: "completed" });
    } catch (e) {
      notify(`erreur : ${e}`, 5000);
      updateActivity(jobId, { phase: String(e), status: "failed" });
    } finally {
      setProgress(null);
      releaseActive(jobId);
    }
  }

  // ---- importing from a memory card ----------------------------------------------
  /** @param {Card} card */
  async function importCard(card) {
    // Prefer the explicitly-chosen import folder; fall back to the primary
    // catalogue root when none is set (preserves the pre-choice behavior).
    let archive = importDir ?? library.root;
    if (!archive) {
      archive = await invoke("pick_folder");
      if (!archive) return;
    }
    setProgress({ verb: "import", done: 0, total: card.raw_count, current: "" });
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
        ? `Import stopped · ${stats.copied} imported`
        : `${stats.copied} imported · ${stats.skipped} skipped · ${stats.failed} failed`;
      notify(summary, 5000);
      invoke("notify_user", { title: "Reveal — import", body: summary }).catch(() => {});
      // A real removable card (has a volume mount point) can now be ejected —
      // the ingest loop's last step, offered in the rail rather than forced.
      if (card.volume) ejectableCard = card;
    } finally {
      setProgress(null);
      importingCard = null;
      cards = await invoke("find_cards");
    }
  }

  function stopImport() {
    invoke("cancel_import").catch(() => {});
  }

  /**
   * Cull one just-imported day-folder down to `aiCullTarget` frames — Rust
   * does prefilter -> vision ranking -> (story marking and/or Desktop
   * export, per the two Settings toggles) in one call (`ai_cull`), so this
   * is just the invocation + error toast; progress comes through the
   * `cull-*` events listened for at boot.
   * @param {string} dir
   * @param {string[]} paths
   */
  async function triggerAiCull(dir, paths) {
    try {
      await invoke("ai_cull", { dir, paths });
      // The Rust side wrote directly to this folder's story note — if it's
      // the one currently open, our in-memory copy is now stale.
      if (dir === library.curDir) {
        storySet = new Set(await invoke("story_stems", { dir }));
        await loadStory();
        refreshStoryDirs();
      }
    } catch (error) {
      notify(`AI Culling (${dir.split("/").pop()}) : ${error}`, 6000);
    }
  }

  /**
   * The rail's manual "AI Culling" button: score the CURRENT folder's view
   * (same prefilter + vision ranking as the walk-away flow, via
   * `ai_cull_selection`) but instead of rating+exporting, add each pick to
   * the folder's quick collection — the same story-note mechanism the `q`
   * shortcut toggles. Only ADDS (never removes) — a photo already in the
   * collection is left alone rather than toggled out.
   */
  async function cullCurrentFolder() {
    if (applePhotosActive) {
      hold("AI culling is available for filesystem folders, not Apple Photos.");
      return;
    }
    const d = library.dir;
    if (!d || !view.length || activity.progress) return;
    setProgress({ verb: "cull", done: 0, total: view.length, current: "" });
    hold(`AI Culling · ${view.length} photos…`);
    cullTaskId = startActivity("cull", `AI Culling · ${view.length} photos`, view.length);
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
      notify(`AI Culling ✓ ${added} added to the quick collection (${result.picked.length}/${result.considered} kept)`, 6000);
      updateActivity(cullTaskId, {
        done: result.picked.length, total: result.considered,
        current: `${added} added`, phase: "Complete", status: "completed",
      });
    } catch (error) {
      notify(`AI Culling : ${error}`, 6000);
      updateActivity(cullTaskId, { phase: String(error), status: "failed" });
    } finally {
      setProgress(null);
      releaseActive(cullTaskId);
      cullTaskId = null;
    }
  }

  /** @param {Card} card */
  async function ejectCard(card) {
    if (!card?.volume || ejecting) return;
    ejecting = true;
    try {
      await invoke("eject_card", { volume: card.volume });
      ejectableCard = null;
      notify(`${card.name} ejected · you can remove the card`, 5000);
    } catch (e) {
      notify(`Eject failed: ${typeof e === "string" ? e : String(e)}`, 6000);
    } finally {
      ejecting = false;
      cards = await invoke("find_cards");
    }
  }

  // ---- exporting -----------------------------------------------------------------
  function saveExportPrefs() {
    session.setExportPrefs({ edge: exportEdge, border: exportBorder, folder: exportFolder });
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

  async function cancelExportQueue() {
    if (!activity.activeId) return;
    updateActivity(activity.activeId, { phase: "Cancelling after current photo…" });
    await invoke("cancel_exports");
  }

  async function exportGrid() {
    if (!view.length) return;
    const jobId = startActivity("export", `Export ${view.length} photos`, view.length);
    try {
      const completed = await invoke("export_photos", {
        paths: view.map((f) => f.path),
        destDir: exportFolder,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      const job = activity.queue.find((item) => item.id === jobId);
      if (job?.status !== "cancelled") {
        updateActivity(jobId, {
          current: "",
          done: completed,
          phase: completed === view.length ? "Complete" : "Stopped",
          status: completed === view.length ? "completed" : "cancelled",
        });
      }
    } catch (error) {
      updateActivity(jobId, {
        phase: String(error),
        status: "failed",
      });
    } finally {
      releaseActive(jobId);
    }
  }

  // The Swift `r` — "Reveal": develop + export the current selection (or the
  // focused frame if nothing is multi-selected) with the active export params.
  async function exportSelection() {
    const targets = selectedFrames(view);
    if (!targets.length || activity.anyRunning) return;
    const jobId = startActivity(
      "export",
      `Export ${targets.length} photo${targets.length > 1 ? "s" : ""}`,
      targets.length,
    );
    try {
      const completed = await invoke("export_photos", {
        paths: targets.map((f) => f.path),
        destDir: exportFolder,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      updateActivity(jobId, {
        current: "",
        done: completed,
        phase: completed === targets.length ? "Complete" : "Stopped",
        status: completed === targets.length ? "completed" : "cancelled",
      });
    } catch (error) {
      updateActivity(jobId, { phase: String(error), status: "failed" });
    } finally {
      releaseActive(jobId);
    }
  }

  /** @param {string} [destDir] override the configured export folder — used by the dev panel's quick-export-to-Desktop button */
  async function exportCurrent(destDir = exportFolder) {
    if (!photoPath || !recipe) return;
    // startActivity already puts "Export <name>" in the notification stack —
    // a second "Exporting…" elsewhere said the same thing twice.
    const jobId = startActivity("export", `Export ${picked}`, 1);
    try {
      await invoke("export_photo", {
        path: photoPath,
        recipe: { ...recipe },
        destDir,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      updateActivity(jobId, {
        current: picked ?? "",
        done: 1,
        phase: "Complete",
        status: "completed",
      });
      notify("Exported ✓", 2000);
    } catch (e) {
      updateActivity(jobId, { phase: String(e), status: "failed" });
      notify(`Export failed: ${e}`, 5000);
    } finally {
      releaseActive(jobId);
    }
  }

  // obsidian:// is the reliable way to land on a SPECIFIC note — `open
  // <path>` would just hand the .md file to whatever app owns that
  // extension, which isn't necessarily Obsidian, and wouldn't target the
  // right vault if more than one is registered for it.
  /** @param {string} notePath */
  function openInObsidian(notePath) {
    invoke("open_path", { path: `obsidian://open?path=${encodeURIComponent(notePath)}` }).catch(() => {});
  }

  // Develop photo(s) and append to the Obsidian daily note (Logs/yymmdd.md).
  // Filesystem export into the vault attachments folder + daily note append.
  /** @param {string} [targetPath] @param {Recipe | null} [customRecipe] */
  async function exportToDailyNote(targetPath, customRecipe) {
    const target = targetPath || photoPath || view[sel]?.path;
    if (!target) return;
    notify("Vers le journal…", 10000);
    try {
      const rec = customRecipe || (target === photoPath ? recipe : null);
      const notePath = await invoke("export_to_daily_note", {
        path: target,
        recipe: rec ? { ...rec } : null,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      const noteName = notePath.split("/").slice(-2).join("/");
      const filename = target.split("/").pop();
      notify(`Dans le journal → ${noteName} (${filename}) ✓`, 4000);
      openInObsidian(notePath);
    } catch (e) {
      notify(`Journal export failed: ${e}`, 5000);
    }
  }

  /** @param {string} [clickedPath] */
  async function exportSelectionToDailyNote(clickedPath) {
    const targets = selection.paths.size > 0
      ? view.filter((f) => selection.paths.has(f.path)).map((f) => f.path)
      : (clickedPath ? [clickedPath] : (view[sel] ? [view[sel].path] : []));

    if (!targets.length) return;

    if (targets.length === 1) {
      return exportToDailyNote(targets[0]);
    }

    // The activity indicator carries "Journal · N photos" for the duration —
    // no second running message needed.
    const jobId = startActivity("publish", `Journal · ${targets.length} photos`, targets.length);
    try {
      const notePath = await invoke("export_batch_to_daily_note", {
        paths: targets,
        longEdge: exportEdge,
        borderFrac: exportBorder ? 0.04 : 0,
      });
      const noteName = notePath.split("/").slice(-2).join("/");
      notify(`Dans le journal → ${targets.length} photos dans ${noteName} ✓`, 4000);
      updateActivity(jobId, { done: targets.length, current: noteName, phase: "Complete", status: "completed" });
      openInObsidian(notePath);
    } catch (e) {
      notify(`Journal export failed: ${e}`, 5000);
      updateActivity(jobId, { phase: String(e), status: "failed" });
    } finally {
      releaseActive(jobId);
    }
  }

  // ---- opening a folder, and the URLs its photos are drawn from ------------------
  async function pickFolder() {
    const path = await invoke("pick_folder");
    if (path) openFolder(path);
  }

  /** @param {string} path */
  async function openFolder(path) {
    leaveApplePhotos();
    // This path had NO guard at all, so a slow listing could land long after
    // you had moved on. Same transaction as openDir now.
    const open = beginOpen({ folder: path });
    // Folder switch always starts with the full contact sheet.
    minRating = 0;
    filterStory = false;
    previewFilter = false;
    const rows = await withPreviewVersions(await invoke("list_dir", { path }));
    if (!open.commit(rows)) return; // overtaken by another folder
    open.finish();
    focusAt(view, 0);
    selectOnly(view, 0);
    // The other way into a folder, and it has to remember as much as the
    // first: without its own session this folder's mode would silently stop
    // being saved. That asymmetry was invisible while both paths wrote to a
    // shared `localStorage` by hand.
    folderSession = openFolderSession(path);
    currentScrollTop = folderSession.scroll || scrollOffsets[path] || 0;
    {
      // A stale "story" value from before Editorial became a filter is
      // rejected by the session's own validation and falls through to "cull"
      // — the grid, with Editorial off, is correct either way.
      const savedMode = folderSession.mode;
      if (savedMode) {
        await switchMode(/** @type {"dev" | "cull"} */ (savedMode));
      } else {
        await switchMode("cull");
      }
    }
    refreshStory();
  }

  /**
   * @param {string} path
   * @param {number} [version]
   */
  // `size` is what the protocol actually serves, and it matters: the grid
  // shows ~120 cells at once, so a 2048px JPEG per cell meant the webview
  // held gigabytes of decoded bitmaps and stuttered on every scroll. Cells
  // ask small.
  //
  // This used to add: "the develop viewer asks for the real thing … there's
  // no reason to look at a blown-up 640px proxy for the seconds the RAW
  // decode takes." That held while the blur lasted seconds. It no longer
  // does — the grid-size copy is in the local cache and paints in under a
  // millisecond, while the 2048 behind it arrives from the NAS in ~55ms
  // measured. Opening now goes cache → sidecar → RAW render, three steps
  // that each replace a blurrier one, rather than one wait.

  /** The full-resolution developed sidecar, for single-photo views.
   * @param {string} path @param {number} [version] */
  function previewUrl(path, version = 0) {
    return `${thumbUrl(path, version, 2048)}&priority=1`;
  }
  /** The grid-size copy for a single-photo view.
   *
   * Deliberately the SAME url the grid cell used, `priority=1` and all: a
   * different string would miss the webview cache and refetch pixels that
   * are already painted a few centimetres away. The queue-jumping matters on
   * a restore, where the grid is not mounted at all and this is the only
   * request in flight.
   * @param {string} path @param {number} [version] */
  function openingUrl(path, version = 0) {
    return thumbUrl(path, version);
  }


  /** @param {number} n */
  async function rate(n) {
    const targets = selectedFrames(view);
    if (!targets.length) return;
    try {
      for (const frame of targets) {
        await invoke("set_rating", { path: frame.path, rating: n });
        frame.rating = n;
      }
    } catch (error) {
      hold(`Could not save photo rating: ${error}`);
    } finally {
      refreshFrames();
    }
  }

  /** @param {string} path */
  /** @param {string} path */
  function showCopiedMessage(path) {
    const filename = path.split("/").pop();
    notify(`Image copied to the clipboard (${filename}) ✓`, 2500);
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
      notify(`Failed to copy the image: ${err}`, 3000);
    }
  }

  // ---- copying a recipe from one photo to others ---------------------------------
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

    notify(`Settings copied from ${source.name}`, 2000);
  }

  // Render + persist one recipe onto a set of frames, updating the live loupe
  // if the open photo is among them. Shared by paste-settings and preset-apply.
  /**
   * @param {Recipe | null} recipeToApply
   * @param {Frame[]} targetFrames
   */
  async function applyRecipeToFrames(recipeToApply, targetFrames) {
    if (!recipeToApply || !targetFrames.length || activity.progress) return;
    const snapshot = { ...recipeToApply };
    setProgress({ verb: "Applying settings", done: 0, total: targetFrames.length, current: "" });
    const jobId = startActivity("develop", `Synchroniser ${targetFrames.length} photo(s)`, targetFrames.length);
    try {
      for (const [i, frame] of targetFrames.entries()) {
        patchProgress({ current: frame.name });
        updateActivity(jobId, { current: frame.name });

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

        // Start this photo's render, then immediately start READING the next
        // one, and only then wait. Measured on this NAS, a frame costs ~3.1s
        // to read and ~1.0s to decode before anything renders; done strictly
        // one after another the link sits idle through every decode and
        // render. Overlapping them means photo N+1 is already in the decode
        // cache by the time the loop reaches it. Fire-and-forget on purpose —
        // a failed prefetch just means the next turn pays what it pays today.
        const developing = invoke("develop_preview", {
          path: frame.path,
          recipe: frameRecipe,
          maxPx: PREVIEW_PX,
        });
        const upcoming = targetFrames[i + 1];
        if (upcoming?.path) invoke("prefetch_photo", { path: upcoming.path }).catch(() => {});
        const bytes = await developing;
        await invoke("save_recipe", { path: frame.path, recipe: frameRecipe });
        frame.previewVersion = await freshPreviewVersion(frame.path);
        updateActivity(jobId, { done: advanceProgress() });

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
        refreshFrames(); // per photo, so grid thumbs update live as each frame develops
      }
      notify(`Settings applied to ${targetFrames.length} photo${targetFrames.length === 1 ? "" : "s"}`, 2500);
      updateActivity(jobId, { phase: "Complete", status: "completed" });
    } catch (e) {
      notify(`Could not apply settings: ${e}`);
      updateActivity(jobId, { phase: String(e), status: "failed" });
    } finally {
      setProgress(null);
      releaseActive(jobId);
    }
  }

  async function pasteSettings() {
    if (!copiedRecipe) return;
    await applyRecipeToFrames(copiedRecipe, selectedFrames(view));
  }

  // ---- the keyboard --------------------------------------------------------------
  // `onKey` is 262 lines and the largest function in the file. It reads as a
  // table because that is what it is; the decisions behind the mode keys live
  // in $lib/controllers/AppController.js, which returns an action this applies.

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
    if (res.action === "EXIT_FULLSCREEN") {
      exitFullscreen();
      return true;
    }
    if (res.action === "TOGGLE_PREVIEW") {
      if (res.andSwitchToCull) {
        if (currentMode !== "cull") switchMode("cull", { openDevPanel: false });
        togglePreviewFilter(true);
      } else {
        togglePreviewFilter();
      }
      return true;
    }
    if (res.action === "GO_TO_GRID") {
      togglePreviewFilter(false);
      if (currentMode !== "cull") switchMode("cull", { openDevPanel: false });
      return true;
    }
    return false;
  }

  /** @param {KeyboardEvent} e */
  function onKey(e) {
    // Shared surfaces own their keys. Never let a menu/dialog keystroke also
    // navigate photos, change mode, or dispatch an export.
    if (e.defaultPrevented || document.querySelector("dialog[open]") ||
      (e.target instanceof Element && e.target.closest("[role='menu'], [role='dialog'], [contenteditable='true']"))) return;
    if (["INPUT", "SELECT", "TEXTAREA"].includes(/** @type {HTMLElement} */ (e.target).tagName)) return;
    // Catalogue/disclosure buttons own native activation. Space/Enter must
    // not simultaneously open a photo or enter quick look/fullscreen.
    if (["Enter", " "].includes(e.key) && e.target instanceof Element &&
      e.target.closest("button, [role='button']")) return;

    // Every mode-dependent decision below reads through this controller
    // instead of comparing `currentMode` inline — one place to look when a
    // shortcut behaves differently per mode, and one place to extend when a
    // new mode shows up.
    const controller = new AppController({
      currentMode,
      spaceLook,
      devPanel: layouts.dev.devPanel
    });

    // ⌘A / Ctrl-A — select all
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a") {
      selectAll(view);
      e.preventDefault();
      return;
    }

    // ⌘C / Ctrl-C — copy photo image to OS clipboard
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "c") {
      const targetPath = controller.resolveCopyTarget({ photoPath, selectedFramePath: view[sel]?.path });
      if (targetPath) {
        copyImageToClipboard(targetPath);
        e.preventDefault();
        return;
      }
    }

    // ⌘D / Ctrl-D — deselect all (clear the focused frame + the ⌘-click set).
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "d") {
      clearSelection();
      e.preventDefault();
      return;
    }

    // ⌘Z — undo the last settled develop edit; ⌘⇧Z — redo. Develop only,
    // scoped to whichever photo is open (see edited()/openPhoto).
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
      if (e.shiftKey) redoRecipeEdit();
      else undoRecipeEdit();
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
    if (e.key.toLowerCase() === "z" && !e.metaKey && !e.ctrlKey && controller.canCycleZoom()) {
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
      // still surfaces automatically (startActivity opens it) and stays
      // reachable from the native menu.
      exportSelection();
      e.preventDefault();
      return;
    }

    // Mode switcher shortcuts handled via AppController
    if (e.key === "g") {
      applyWorkflowResult(controller.handleG({ previewFilter }));
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
      applyWorkflowResult(controller.handleEscape({ hasOverlay: false, fullscreen, previewFilter }));
      e.preventDefault();
      return;
    }

    // In Develop mode (single photo view), ArrowUp / ArrowDown modifies the last edited setting!
    if (controller.canAdjustRecipe() && (e.key === "ArrowUp" || e.key === "ArrowDown") && !e.metaKey && !e.ctrlKey) {
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

    const c = controller.navColumnCount({ fullscreen, cols });
    const isNav = ["ArrowRight", "ArrowLeft", "ArrowDown", "ArrowUp"].includes(e.key);
    
    if (isNav) {
      let nextSel = sel;
      if (e.key === "ArrowRight") nextSel = Math.min(sel + 1, view.length - 1);
      else if (e.key === "ArrowLeft") nextSel = Math.max(sel - 1, 0);
      else if (e.key === "ArrowDown") nextSel = Math.min(sel + c, view.length - 1);
      else if (e.key === "ArrowUp") nextSel = Math.max(sel - c, 0);
      
      focusAt(view, nextSel);

      if (e.shiftKey) {
        selectRange(view, selectionAnchor, sel);
      } else if (e.metaKey || e.ctrlKey) {
        // macOS/Windows pattern: Cmd/Ctrl + Arrow just moves the cursor (sel) without changing selection.
      } else {
        selectOnly(view, sel);
      }
      
      e.preventDefault();
      if (fullscreen) {
        prepareFullscreenFrame(view[sel].path);
      } else if (controller.shouldOpenOnNav() && view[sel]) {
        openPhoto(view[sel].path);
      }
      document.querySelector(`[data-idx="${sel}"]`)?.scrollIntoView({ block: "nearest" });
      return;
    }

    if (e.key === " " && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
      if (controller.isCull()) {
        // Guard only — do NOT assign photoPath here. `photoPath` means "the
        // photo currently LOADED in Develop", and switchMode uses exactly
        // that to decide whether it must open a different one:
        //
        //   if (view[sel] && (photoPath !== view[sel].path || !recipe))
        //
        // Setting it first made that condition false, so arrowing to another
        // photo and pressing space switched to Develop still showing the
        // previous photo's pixels and recipe under the new photo's name
        // (Francis, 2026-09-22). `handleSpace()` never reads photoPath.
        if (!selection.paths.size && !view[sel]) return;
        if (!view[sel]?.path) return;
      }
      applyWorkflowResult(controller.handleSpace());
      e.preventDefault();
      return;
    }
    // Toggle selection with space when holding Cmd/Ctrl (like macOS Finder)
    else if (e.key === " " && (e.metaKey || e.ctrlKey)) {
      if (view[sel]) {
        toggleSelected(view[sel].path);
        // The toggled photo becomes the anchor, so a following shift-arrow
        // extends from it.
        setAnchor(view[sel].path);
      }
      e.preventDefault();
      return;
    }
    else if (e.key >= "0" && e.key <= "5") rate(Number(e.key));
    else return;

    e.preventDefault();
  }

  // ---- developing the open photo -------------------------------------------------
  /** @param {string} path */
  /**
   * @param {string} path
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  // Decode the frames around the open one into the engine's cache while the
  // user is still looking at this one. On a NAS-hosted library a step to the
  // next photo is ~3s of network read plus ~1s of decode; doing it ahead of
  // the request is the difference between a cull that flows and one that
  // stutters. Next first — that's the direction a cull actually moves.
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let prefetchTimer;
  /** @param {string} path */
  function prefetchNeighbours(path) {
    if (!isTauri) return;
    // Debounced, and deliberately not immediate: holding the arrow key
    // through twenty frames would otherwise queue forty decodes competing
    // with the one photo actually on screen. Waiting until the user pauses
    // means we prefetch around where they've settled.
    clearTimeout(prefetchTimer);
    prefetchTimer = setTimeout(() => {
      if (photoPath !== path) return; // moved on again
      const i = view.findIndex((f) => f.path === path);
      if (i < 0) return;
      for (const n of [view[i + 1], view[i - 1]]) {
        if (n?.path) invoke("prefetch_photo", { path: n.path }).catch(() => {});
      }
    }, 450);
  }

  // Same debounce, aimed at a different moment: the photo highlighted in the
  // GRID, so stepping into Develop finds its decode already warm.
  //
  // The delay is the whole design. The decode cache is bounded by bytes
  // (3 GB of ProPhoto f32 — roughly forty frames from a high-megapixel body),
  // and a cull moves the selection several times a second. Warming on every
  // keystroke would evict what you just looked at and hammer the NAS for
  // frames you are only passing over. Warming when you STOP costs one decode,
  // and it is the one that pays off.
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let selectionWarmTimer;
  /** @param {string} path */
  function warmSelection(path) {
    if (!isTauri || !path) return;
    clearTimeout(selectionWarmTimer);
    selectionWarmTimer = setTimeout(() => {
      // Develop does its own warming, and knows more than this does.
      if (currentMode !== "cull") return;
      if (view[sel]?.path !== path) return; // moved on again
      invoke("prefetch_photo", { path }).catch(() => {});
    }, 450);
  }

  $effect(() => {
    const path = view[sel]?.path;
    if (path) untrack(() => warmSelection(path));
  });

  // Parking a decode costs a 35 MB write; releasing it throws that away.
  // Doing either on every mode switch turned grid → dev → grid → dev into
  // write-delete-write-delete, and the decodes behind it kept six cores busy
  // long after the navigation stopped (Francis, 2026-09-22: "j'ai passé d'une
  // photo à l'autre rapidement... grille dev, grille dev").
  //
  // So both wait to see whether you meant it. Flipping back and forth now
  // does no work at all: the pending action is simply cancelled, and
  // returning to the photo already parked finds it still there.
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let workingParkTimer;
  /** The photo currently parked on disk, so re-entering it is a no-op.
   * @type {string | null} */
  let parkedPath = null;

  /** @param {string} path */
  function scheduleWorkingPark(path) {
    clearTimeout(workingParkTimer);
    if (!isTauri || parkedPath === path) return;
    workingParkTimer = setTimeout(() => {
      if (photoPath !== path || currentMode !== "dev") return; // moved on
      parkedPath = path;
      invoke("park_working_frame", { path }).catch(() => (parkedPath = null));
    }, 1500);
  }

  function scheduleWorkingRelease() {
    clearTimeout(workingParkTimer);
    if (!isTauri || !parkedPath) return;
    workingParkTimer = setTimeout(() => {
      if (currentMode === "dev") return; // came back
      parkedPath = null;
      invoke("release_working_frame").catch(() => {});
    }, 5000);
  }

  /**
   * @param {string} path
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  async function openPhoto(path, { openDevPanel = true } = {}) {
    if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
    photoPath = path;
    picked = path.split("/").pop() ?? null;
    // Step one of three: the grid-size copy, which the local cache almost
    // always holds already. Step two swaps in the 2048 sidecar below; step
    // three is the RAW render from `pump()`.
    const openVersion = library.frames.find((f) => f.path === path)?.previewVersion ?? 0;
    imgUrl = openingUrl(path, openVersion);
    useCanvas = false; // start on the <img> thumb; pump() flips this back on
    // only if the photo develops with a canvas (Rapid) engine.
    // Step two: the real 2048. Only applied if the user is still on this
    // photo and nothing better (a canvas render) has taken over since.
    const openPath = path;
    queueMicrotask(() => {
      const full = new Image();
      full.onload = () => {
        if (photoPath === openPath && !useCanvas) imgUrl = full.src;
      };
      full.src = previewUrl(openPath, openVersion);
    });
    imgFailed = false;
    status = "";
    try {
      const [sidecar, defaults] = await Promise.all([
        invoke("load_sidecar", { path }),
        invoke("default_recipe"),
      ]);
      if (path !== photoPath) return;
      caption = sidecar?.description ?? "";
      tags = sidecar?.tags ?? [];
      developEngine = sidecar?.engine
        ? sidecar.engine
        : (sidecar?.engine_settings ? "spektra" : (preferences.default_engine || null));
      recipe = {
        ...defaults,
        ...(sidecar?.engine_settings ?? {}),
        ...(!sidecar?.engine && !sidecar?.engine_settings && preferences.default_engine
          ? { engine: preferences.default_engine }
          : {}),
      };
      // A new photo starts its own undo history — edits to the last one
      // don't bleed into this one, and vice versa.
      recipeUndoStack = [];
      recipeRedoStack = [];
      lastCommittedRecipe = snapshotRecipe(recipe);
      prefetchNeighbours(path);
      scheduleWorkingPark(path);
      if (developEngine) {
        scheduleRender(PREVIEW_PX);
      } else {
        // Engine "None": show the as-shot preview (the camera JPEG / the
        // .reveal.jpg the grid already shows via reveal://thumb), not a
        // color-developed RAW. Touching a control re-engages the engine via
        // edited(). Also avoids polluting the .reveal.jpg cache.
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
      notify(`Could not open photo: ${error}`, 5000);
    }
  }

  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let captionTimer = undefined;
  function captionEdited() {
    clearTimeout(captionTimer);
    const path = photoPath;
    const description = caption;
    captionTimer = setTimeout(() => {
      if (path) invoke("save_caption", { path, description })
        .catch((error) => { hold(`Could not save caption: ${error}`); });
    }, 400);
  }

  function tagsEdited() {
    const path = photoPath;
    if (!path) return;
    invoke("save_tags", { path, tags: [...tags] })
      .catch((error) => { hold(`Could not save tags: ${error}`); });
  }

  let inflight = $state(false);
  /** @type {number | null} */ let pendingPx = $state(null);

  /** Whether the pending render is a live drag frame rather than a settled
   * edit. Travels with `pendingPx` so latest-wins keeps them consistent: a
   * settle arriving after a drag replaces both. The backend cannot infer this
   * from the pixel size any more — Rapid on the GPU drags at full 2048.
   * @type {boolean} */
  let pendingLive = false;
  /**
   * Read a frame off the raw IPC channel: four u32 of header, then the pixels.
   *
   * The pixels are a VIEW into the transferred buffer, not a copy — that is
   * the whole point of the change on the Rust side. Building an
   * `ArrayBuffer` here from a JSON array of 11 million numbers is exactly
   * what used to make a slider drag feel heavy.
   * @param {ArrayBuffer | ArrayBufferView} buf
   * @returns {RgbaPreview}
   */
  function unpackFrame(buf) {
    const ab = /** @type {ArrayBuffer} */ (
      ArrayBuffer.isView(buf)
        ? buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength)
        : buf
    );
    const head = new DataView(ab, 0, 16);
    const width = head.getUint32(0, true);
    const height = head.getUint32(4, true);
    const renderMs = head.getUint32(8, true);
    return {
      width,
      height,
      renderMs,
      rgba: new Uint8ClampedArray(ab, 16, width * height * 4),
    };
  }

  /** @param {number} px @param {boolean} [live] */
  function scheduleRender(px, live = false) {
    pendingPx = px; // latest wins
    pendingLive = live;
    pump();
  }

  async function pump() {
    if (inflight || pendingPx === null || !photoPath || !recipe) return;
    inflight = true;
    const px = pendingPx;
    const live = pendingLive;
    pendingPx = null;
    pendingLive = false;
    const path = photoPath;
    const snap = { ...recipe };
    const t0 = performance.now();
    try {
      if (snap.engine === "rapid") {
        const res = unpackFrame(
          await invoke("develop_preview_rgba", { path, recipe: snap, maxPx: px, live }),
        );
        if (path === photoPath) {
          renderMs = Math.round(performance.now() - t0);
          useCanvas = true;
          // Reactive aspect for the canvas path — canvasEl.width is a DOM
          // mutation the view can't track, so the mat sizing (breathing room)
          // would never apply and the margins broke. Feed it explicitly.
          //
          // Only from a SETTLED (>= PREVIEW_PX) render, though — a live drag
          // renders at the much smaller DRAG_PX budget, whose width/height
          // round to a very slightly different ratio than the settled
          // render even for the exact same crop. That was enough to nudge
          // the CSS aspect-ratio box's on-screen size every single edit
          // (drag starts → box flickers to the drag-res ratio → settles →
          // flickers back). Leaving renderAspect alone during a drag keeps
          // the box locked to its last settled size; the smaller canvas
          // just stretches to fill it (softer, not smaller) via the
          // existing object-fit, exactly what was asked for.
          // `!live`, not `px >= PREVIEW_PX`: the pixel size stopped meaning
          // "settled" when Rapid on the GPU began dragging at full 2048. It
          // still happens to be harmless here (same size, same ratio), but
          // the same stale proxy did real damage one branch over.
          if (!live || renderAspect === null) {
            renderAspect = res.height ? res.width / res.height : null;
          }
          await tick();
          if (canvasEl) {
            canvasEl.width = res.width;
            canvasEl.height = res.height;
            const ctx = canvasEl.getContext("2d");
            if (ctx) {
              const imgData = new ImageData(res.rgba, res.width, res.height);
              ctx.putImageData(imgData, 0, 0);
              canvasVersion++;
            }
          }
          imgFailed = false;
          status = "";
          const frame = library.frames.find((item) => item.path === path);
          if (frame && px >= PREVIEW_PX) {
            frame.previewVersion = await freshPreviewVersion(frame.path);
            refreshFrames();
          }
        }
      } else {
        // NOT `useCanvas = false` here. Clearing before the render means
        // switching Rapid → Spektra blanks the photo for the whole ~2.5s the
        // film simulation takes, and the busy spinner only appears after
        // 400ms — so what you actually see is an empty frame (Francis:
        // "passer de rapid à spektra me fais d'abord enlever la photo pour
        // attendre"). The previous render stays up until there is something
        // better to put in its place.
        const bytes = await invoke("develop_preview", { path, recipe: snap, maxPx: px });
        const url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
        const img = new Image();
        img.src = url;
        try { await img.decode(); } catch (e) {}

        if (path === photoPath) {
          renderMs = Math.round(performance.now() - t0);
          if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
          imgUrl = url;
          // Swap the surface only now, with the new pixels decoded and ready.
          useCanvas = false;
          imgFailed = false;
          const frame = library.frames.find((item) => item.path === path);
          if (frame) {
            frame.previewVersion = await freshPreviewVersion(frame.path);
            refreshFrames();
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
    // Undo history only advances on a settled commit — a slider dragging
    // through 40 intermediate values (`live: true`) is ONE undo step, not
    // 40. `restoringRecipeHistory` skips this while undo/redo itself is
    // calling edited() to apply a restored snapshot, so restoring never
    // re-pushes its own predecessor.
    if (!live && !restoringRecipeHistory && recipe) {
      if (lastCommittedRecipe) {
        recipeUndoStack.push(lastCommittedRecipe);
        if (recipeUndoStack.length > MAX_RECIPE_UNDO_STEPS) recipeUndoStack.shift();
      }
      recipeRedoStack = [];
      lastCommittedRecipe = snapshotRecipe(recipe);
    }
    scheduleRender(liveRenderPx(live), live);
    clearTimeout(saveTimer);
    const path = photoPath;
    const snapshot = recipe ? { ...recipe } : null;
    saveTimer = setTimeout(() => {
      if (path && snapshot) invoke("save_recipe", { path, recipe: snapshot })
        .catch((error) => { hold(`Could not save development settings: ${error}`); });
    }, 300);
  }

  function undoRecipeEdit() {
    if (!recipeUndoStack.length || !recipe || currentMode !== "dev") return;
    const previous = /** @type {Recipe} */ (recipeUndoStack.pop());
    recipeRedoStack.push(snapshotRecipe(recipe));
    restoringRecipeHistory = true;
    recipe = previous;
    developEngine = previous.engine === "rapid" ? "rapid" : (previous.engine ? "spektra" : developEngine);
    lastCommittedRecipe = snapshotRecipe(recipe);
    edited(false);
    restoringRecipeHistory = false;
  }

  function redoRecipeEdit() {
    if (!recipeRedoStack.length || !recipe || currentMode !== "dev") return;
    const next = /** @type {Recipe} */ (recipeRedoStack.pop());
    recipeUndoStack.push(snapshotRecipe(recipe));
    restoringRecipeHistory = true;
    recipe = next;
    developEngine = next.engine === "rapid" ? "rapid" : (next.engine ? "spektra" : developEngine);
    lastCommittedRecipe = snapshotRecipe(recipe);
    edited(false);
    restoringRecipeHistory = false;
  }

  async function clearDevelopment() {
    if (!photoPath) return;
    const path = photoPath;
    await invoke("clear_recipe", { path });
    if (path !== photoPath) return;
    developEngine = null;
    const frame = library.frames.find((item) => item.path === path);
    if (frame) frame.previewVersion = await freshPreviewVersion(frame.path);
    refreshFrames();

    // Decode the as-shot preview BEFORE swapping surfaces. Flipping
    // `useCanvas` first would hide a good Rapid render and show an empty
    // <canvas> while the <img> fetches — the same blank-then-fill the engine
    // switch used to do.
    const next = previewUrl(path, frame?.previewVersion ?? Date.now());
    const probe = new Image();
    probe.src = next;
    try { await probe.decode(); } catch { /* show it anyway; onerror handles it */ }
    if (path !== photoPath) return;
    if (imgUrl?.startsWith("blob:")) URL.revokeObjectURL(imgUrl);
    imgUrl = next;
    useCanvas = false;
    status = "";
  }

  // ---- the docked Develop panel's controls ---------------------------------------
  // Same mutation logic the detached panel runs locally before its emit
  // (dev-panel/+page.svelte) — here there's nothing to emit, `edited` above
  // already IS the notification (it schedules the render and the disk save).

  /** @param {boolean} [transient] @param {string} [key] */
  function dockedEdited(transient = false, key = undefined) {
    if (key) lastEditedKey = key;
    edited(transient);
  }

  // Recipe's JSDoc typedef lists its known fields for the rest of the app;
  // these functions poke it by dynamic key (LUT stacks, per-engine controls),
  // same as the detached panel's own copies — an `any` view is the honest
  // type for that, not a workaround.
  /** @param {string} key @param {number | string} v @param {boolean} transient @param {number} [index] */
  function setDevNum(key, v, transient, index) {
    if (!recipe) return;
    const r = /** @type {any} */ (recipe);
    if (index != null) {
      if (!Array.isArray(r[key])) r[key] = [];
      r[key][index] = Number(v);
    } else {
      r[key] = Number(v);
    }
    dockedEdited(transient, key);
  }

  /** @param {string} key @param {number} [index] */
  function resetOne(key, index) {
    const d = /** @type {any} */ (developDefaults);
    if (!d || !recipe || !(key in d)) return;
    if (index != null) {
      setDevNum(key, d[key]?.[index] ?? 0, false, index);
    } else {
      setDevNum(key, d[key], false);
    }
  }

  /** @param {string} stage */
  const lutsKey = (stage) => (stage === "pre" ? "rapid_pre_luts" : "rapid_post_luts");
  /** @param {string} stage */
  const oldLutsKey = (stage) => (stage === "pre" ? "pre_luts" : "post_luts");
  /** @param {any} r @param {string} stage */
  function ensureLutMigration(r, stage) {
    if (!r || developEngine !== "rapid") return;
    const key = lutsKey(stage);
    const oldKey = oldLutsKey(stage);
    if (r[oldKey]?.length > 0) {
      r[key] = [...(r[key] ?? []), ...r[oldKey]];
      r[oldKey] = [];
      dockedEdited();
    }
  }
  /** @param {string} stage */
  function addLutLayer(stage) {
    if (!recipe) return;
    const r = /** @type {any} */ (recipe);
    ensureLutMigration(r, stage);
    const key = lutsKey(stage);
    const first = typeof luts[0] === "string" ? luts[0] : (luts[0]?.name ?? "");
    r[key] = [...(r[key] ?? []), { name: first, opacity: 1 }];
    dockedEdited();
  }
  /** @param {string} stage @param {number} index */
  function removeLutLayer(stage, index) {
    if (!recipe) return;
    const r = /** @type {any} */ (recipe);
    ensureLutMigration(r, stage);
    const key = lutsKey(stage);
    r[key] = (r[key] ?? []).filter((/** @type {any} */ _, /** @type {number} */ i) => i !== index);
    dockedEdited();
  }
  /** @param {string} stage @param {number} index @param {number | string} value */
  function updateLutOpacity(stage, index, value) {
    if (!recipe) return;
    const r = /** @type {any} */ (recipe);
    ensureLutMigration(r, stage);
    const key = lutsKey(stage);
    r[key] = (r[key] ?? []).map((/** @type {any} */ l, /** @type {number} */ i) => (i === index ? { ...l, opacity: Number(value) } : l));
    dockedEdited(true);
  }
  /** @param {string} stage @param {number} index @param {string} name */
  function setLutFile(stage, index, name) {
    if (!recipe) return;
    const r = /** @type {any} */ (recipe);
    ensureLutMigration(r, stage);
    const key = lutsKey(stage);
    r[key] = (r[key] ?? []).map((/** @type {any} */ l, /** @type {number} */ i) => (i === index ? { ...l, name } : l));
    dockedEdited();
  }

  /** @param {string | null} engineId */
  function applyEngineChange(engineId) {
    if (engineId === null) {
      clearDevelopment();
    } else {
      developEngine = engineId;
      if (recipe) recipe.engine = developEngine === "rapid" ? "rapid" : "spektra";
      edited(false);
    }
  }
  /** @param {string} value */
  function dockedEngineChanged(value) {
    applyEngineChange(value === "none" ? null : value);
  }

  async function applyResetRecipe() {
    if (recipe) {
      recipe = await invoke("default_recipe");
      edited();
    }
  }

  function dockedToggleClipping() {
    showClipping = !showClipping;
  }

  function dockedToggleCaptionOverlay() {
    showCaption = !showCaption;
  }

  function dockedHidePanel() {
    layouts.dev.devPanel = false;
    saveLayouts();
  }

  // exportEdge/exportBorder are already mutated directly (bind: on
  // <DevelopPanel>) — only the persist step needs doing.
  function dockedExportSettingsChanged() {
    saveExportPrefs();
  }

  /** @param {number | string} v */
  const fmt = (v) => Number(v).toFixed(2).replace(/\.?0+$/, "") || "0";
  /** @param {number} n */
  const stars = (n) => "★".repeat(n);

  // The star menu sets an absolute threshold (Tout / ≥ N★), like Swift's menu.
  /** @param {number} n */
  function setMinRating(n) {
    minRating = n;
    if (library.curDir) openDir(library.curDir);
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
    if ((!library.root && !applePhotosSupported) || layouts[currentMode].sidebar) return;
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

  const sidebarVisible = $derived((!!library.root || applePhotosSupported) && layouts[currentMode].sidebar);
</script>

{#if applePhotosTransfer?.phase === "loading"}
  <div class="photos-transfer" role="status">
    <Alert class="info" title="Apple Photos">
    <span>Preparing {applePhotosTransfer.name} from Apple Photos (iCloud if needed)...</span>
    <button class="btn ghost" onclick={cancelApplePhotosTransfer}>Cancel download</button>
    </Alert>
  </div>
{:else if applePhotosTransfer?.phase === "error"}
  <div class="photos-transfer" role="alert">
    <Alert class="error" title="Apple Photos">
    <span>{applePhotosTransfer.error}</span>
    <button class="btn ghost" onclick={() => { applePhotosTransfer = null; }}>Dismiss</button>
    </Alert>
  </div>
{/if}

<svelte:window onkeydown={onKey} />

{#if isTauri && !fullscreen}
  <div
    class="window-controls-zone"
    class:dev={currentMode === "dev"}
  >
    <div
      class="window-controls"
      class:dev={currentMode === "dev"}
      aria-label="Window controls"
    >
      <button class="window-close" onclick={closeMainWindow} aria-label="Close window"></button>
      <button class="window-minimize" onclick={minimizeMainWindow} aria-label="Minimize window"></button>
      <button class="window-zoom" onclick={zoomMainWindow} aria-label="Zoom window"></button>
    </div>
  </div>
{/if}

{#if currentMode === "cull"}
  <div
    class="cull"
    class:themed={appThemed}
    role="presentation"
    style:--color-background={appBg}
    style:--color-foreground={appFg}
    style:--color-accent={appAccent}
    style:--font-header={appFontHeader}
    style:--font-text={appFontText}
    style:--font-ratio={appFontRatio}
    style:--radius={appMeasuredRadius["--radius"]}
    style:--radius-sm={appMeasuredRadius["--radius-sm"]}
    style:--radius-lg={appMeasuredRadius["--radius-lg"]}
  >
    <div class="body">
      {#if sidebarVisible}
        <Sidebar
          applePhotos={applePhotosLibrary}
          onConnectApplePhotos={connectApplePhotos}
          onRefreshApplePhotos={refreshApplePhotos}
          root={library.root}
          roots={library.roots}
          dirs={library.dirs}
          curDir={library.curDir}
          {scanning}
          {indexProgress}
          {previewFilter}
          {storyDirs}
          focusOn={layouts[currentMode].focus}
          {catalogContent}
          {importDir}
          onOpenDir={openDir}
          onOpenLibrary={() => openDir(library.root)}
          onRescan={rescan}
          onRescanDir={rescanDir}
          onRevealDir={revealDir}
          onAddLocation={indexRoot}
          onRemoveLibrary={removeLibrary}
          onSetImportDir={setImportDir}
          onOpenNote={() => (catalogOpen = true)}
          onTogglePreview={() => togglePreviewFilter()}
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
          selectedCount={selection.paths.size || (view[sel] ? 1 : 0)}
          onMoveSelectedPhotos={moveSelectedPhotosToDir}
          onRenameDir={renameDir}
          onCreateFolder={createFolder}
          onMoveDir={moveDir}
          onDevelopStory={exportLocalStory}
          onPublishStory={publishStory}
          onExportLocalStory={exportLocalStory}
          publishing={!!activity.progress}
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
            applePhotos={applePhotosLibrary}
            onConnectApplePhotos={connectApplePhotos}
            onRefreshApplePhotos={refreshApplePhotos}
            root={library.root}
            roots={library.roots}
            dirs={library.dirs}
            curDir={library.curDir}
            {scanning}
            {indexProgress}
            {previewFilter}
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
              openDir(library.root);
              closeSidebarPeek();
            }}
            onRescan={rescan}
            onRescanDir={rescanDir}
            onRevealDir={revealDir}
            onAddLocation={indexRoot}
            onRemoveLibrary={removeLibrary}
            onSetImportDir={setImportDir}
            onOpenNote={() => (catalogOpen = true)}
            onTogglePreview={() => togglePreviewFilter()}
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
            selectedCount={selection.paths.size || (view[sel] ? 1 : 0)}
            onMoveSelectedPhotos={moveSelectedPhotosToDir}
            onRenameDir={renameDir}
            onCreateFolder={createFolder}
            onMoveDir={moveDir}
            onDevelopStory={exportLocalStory}
            onPublishStory={publishStory}
            onExportLocalStory={exportLocalStory}
            publishing={!!activity.progress}
            publishStatus={status}
            {gardenUrl}
          />
        </div>
      {/if}
      <div class="content" role="presentation" onmousedown={startWindowDrag}>
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
                title="Focus mode — dims the background (o)"
              >
                <span class="focus-glyph" class:on={layouts[currentMode].focus}></span>
              </button>
              <button class="chrome-btn" onclick={toggleAppearance} title="Toggle system light / dark mode (l)">
                <Icon name="circle-half" size="12px" />
              </button>
              <button class="wordmark" onclick={() => (shortcutsOpen = true)} title="Keyboard shortcuts">
                {library.curDir && library.curDir !== library.root ? (dirLabel(library.curDir) ?? "").toUpperCase() : "REVEAL"}
              </button>
            </div>
          {/if}

          <!-- Star filter -->
          <Dropdown label="Filter by rating" triggerClass={`rail-btn star-filter-btn ${minRating > 0 || filterStory ? "on" : ""}`}>
            {#snippet trigger()}
              <Icon name="star" size="12px" />
              {#if minRating > 0}
                <span class="rail-badge">{minRating}★</span>
              {:else if filterStory}
                <span class="rail-badge">Q</span>
              {/if}
            {/snippet}
                <DropdownItem
                  role="menuitemcheckbox" aria-checked={filterStory}
                  onclick={() => {
                    filterStory = !filterStory;
                  }}
                  disabled={applePhotosActive}
                >
                  Quick collection
                  {#if filterStory}<Icon name="check" size="10px" />{/if}
                </DropdownItem>
                <DropdownSeparator />
                {#each [0, 1, 2, 3, 4, 5] as n}
                  <DropdownItem role="menuitemradio" aria-checked={minRating === n} onclick={() => setMinRating(n)}>
                    {n === 0 ? "Show all" : `≥ ${n} ★`}
                    {#if minRating === n}<Icon name="check" size="10px" />{/if}
                  </DropdownItem>
                {/each}
          </Dropdown>

          <!-- Sort -->
          <Dropdown label="Sort photos" triggerClass="rail-btn">
            {#snippet trigger()}
              <Icon name="arrows-down-up" size="12px" />
            {/snippet}
                <DropdownItem
                  role="menuitemradio" aria-checked={!sortDesc}
                  onclick={() => {
                    sortDesc = false;
                    saveGridPrefs();
                    if (applePhotosActive) openApplePhotos(applePhotosAlbum);
                  }}
                >
                  Oldest first
                  {#if !sortDesc}<Icon name="check" size="10px" />{/if}
                </DropdownItem>
                <DropdownItem
                  role="menuitemradio" aria-checked={sortDesc}
                  onclick={() => {
                    sortDesc = true;
                    saveGridPrefs();
                    if (applePhotosActive) openApplePhotos(applePhotosAlbum);
                  }}
                >
                  Newest first
                  {#if sortDesc}<Icon name="check" size="10px" />{/if}
                </DropdownItem>
          </Dropdown>

          <span class="frame-count">
            {#if view.length !== frames.length}
              {view.length}/{frames.length} FRAMES
            {:else}
              {view.length} FRAMES
            {/if}
          </span>

          <span class="rail-spacer"></span>
          {#if applePhotosActive}
            <span class="frame-count">{frames.length} / {applePhotosTotal} Apple Photos</span>
            {#if applePhotosOffset < applePhotosTotal}
              <button class="rail-action" onclick={loadMoreApplePhotos} disabled={applePhotosBusy}>
                {applePhotosBusy ? "Loading..." : "Load more photos"}
              </button>
            {/if}
          {/if}

          {#if !library.root && !applePhotosActive}
            <button class="rail-action" onclick={indexRoot} disabled={!isTauri || scanning}>
              {scanning ? "indexing…" : "Index a library"}
            </button>
          {/if}
          <!-- Idle cards: one import button each. The card mid-import shows
               the live chip below instead, so hide its button. -->
          {#each cards as card (card.dcim)}
            {#if !importingCard || importingCard.dcim !== card.dcim}
              <button class="import rail-action" onclick={() => importCard(card)} disabled={!!activity.progress}>
                Import {card.name} ({card.raw_count})
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
              {ejecting ? "Ejecting…" : `Eject ${ejectableCard.name}`}
            </button>
          {/if}

          <!-- The live import chip — same DIN/mono treatment as export, plus
               a stop control; the copy finishes its current file then halts. -->
          {#if activity.progress && activity.progress.verb === "import"}
            <span class="export-chip">
              {#if activity.progress.path}
                <!-- Live preview of the frame being copied (embedded RAW JPEG),
                     keyed on the path so each new file swaps the image. -->
                {#key activity.progress.path}
                  <img class="chip-thumb" src={thumbUrl(activity.progress.path)} alt="" />
                {/key}
              {/if}
              <span class="chip-label">Importing</span>
              <span class="chip-count">{activity.progress.done}/{activity.progress.total}</span>
              <span class="chip-bar">
                <span
                  class="chip-fill"
                  style="width: {activity.progress.total ? (activity.progress.done / activity.progress.total) * 100 : 0}%"
                ></span>
              </span>
              <button class="chip-stop" onclick={stopImport} title="Stop the import">
                <Icon name="x" size="9px" />
              </button>
            </span>
          {/if}

          <!-- The live export chip — DIN label, mono count, a thin accent
               bar filling as frames finish; gone when the batch ends. -->
          {#if activity.progress && activity.progress.verb === "export"}
            <span class="export-chip">
              <span class="chip-label">Developing</span>
              <span class="chip-count">{activity.progress.done}/{activity.progress.total}</span>
              <span class="chip-bar">
                <span
                  class="chip-fill"
                  style="width: {activity.progress.total ? (activity.progress.done / activity.progress.total) * 100 : 0}%"
                ></span>
              </span>
            </span>
          {/if}

          {#if (library.curDir || library.folder) && view.length}
            <button
              class="rail-btn"
              onclick={exportSelection}
              disabled={!!activity.progress}
              title={selection.paths.size > 1 ? (selection.paths.size === view.length ? `Export all photos (${view.length}) (r)` : `Export the ${selection.paths.size} selected photos (r)`) : `Export the selected photo (r)`}
            >
              <Icon name="export" size="12px" />
            </button>
          {/if}

          <!-- The grid's whole geometry behind ONE icon. -->
          <Popover bind:open={layoutMenuOpen} label="Grid layout" align="end">
          {#snippet trigger(/** @type {import('svelte/elements').HTMLButtonAttributes} */ attributes)}
            <button
              class="rail-btn"
              {...attributes}
              aria-label="Grid layout"
              title="Grid layout"
            >
              <Icon name={layout === "masonry" ? "rows" : "grid-four"} size="12px" />
            </button>
          {/snippet}
              <div class="layout-controls">
                {#if gardenUrl}
                  <button
                    class="std-menu-item"
                    onclick={() => {
                      layoutMenuOpen = false;
                      if (gardenUrl) invoke("open_path", { path: gardenUrl });
                    }}
                  >
                    <span class="item-label">Ouvrir sur le web</span>
                    <Icon name="arrow-square-out" size="10px" />
                  </button>
                {/if}
                {#if storySet.size && gardenAccount?.signed_in}
                  <button
                    class="std-menu-item"
                    class:disabled={!!activity.progress}
                    onclick={() => {
                      layoutMenuOpen = false;
                      publishStory();
                    }}
                    disabled={!!activity.progress}
                  >
                    <span class="item-label">Publier l'histoire ({storySet.size})</span>
                    <Icon name="lightning" size="10px" />
                  </button>
                {/if}
                {#if (library.curDir || library.folder) && view.length}
                  <button
                    class="std-menu-item"
                    class:disabled={!!activity.progress}
                    onclick={() => {
                      layoutMenuOpen = false;
                      cullCurrentFolder();
                    }}
                    disabled={!!activity.progress}
                  >
                    <span class="item-label">AI Culling</span>
                    <Icon name="lightning" size="10px" />
                  </button>
                {/if}
                {#if gardenUrl || storySet.size || ((library.curDir || library.folder) && view.length)}
                  <div class="std-menu-separator"></div>
                {/if}
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
                <div class="std-menu-separator"></div>
                <span class="pop-label">Format</span>
                <!-- Disabled rather than clickable-then-refused. The refusal
                     used to be a toast at the bottom of the window, four
                     seconds, while the eye was up here in the menu — Francis
                     never saw it and read the result as masonry being broken. -->
                <button
                  class="std-menu-item"
                  disabled={masonryTooBig}
                  title={masonryTooBig
                    ? `Masonry draws every photo at once; ${library.frames.length} is past what stays smooth (limit ${MASONRY_LIMIT})`
                    : undefined}
                  onclick={toggleLayout}
                >
                  <span class="item-label">Masonry</span>
                  {#if masonryTooBig}
                    <span class="item-note">{library.frames.length} &gt; {MASONRY_LIMIT}</span>
                  {:else if layout === "masonry"}
                    <Icon name="check" size="10px" />
                  {/if}
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
                    class="std-menu-item"
                    onclick={() => {
                      fillCells = !fillCells;
                      saveGridPrefs();
                    }}
                  >
                    <span class="item-label">{fillCells ? "Fill cells" : "Keep aspect ratio"}</span>
                    {#if !fillCells}<Icon name="check" size="10px" />{/if}
                  </button>
                {/if}
                <div class="std-menu-separator"></div>
                <span class="pop-label">Marge</span>
                <input
                  class="pop-slider"
                  type="range"
                  min="0.25"
                  max="6"
                  step="0.25"
                  bind:value={marginScale}
                  aria-label="Grid margin"
                  style="--f: {pct(marginScale, 0.25, 6)}"
                  onchange={saveGridPrefs}
                />
              </div>
          </Popover>
        </header>

      {#if previewFilter}
        <StoryView
          {view}
          {storySet}
          {storyContent}
          progress={activity.progress}
          liveUrl={liveUrl ?? undefined}
          {thumbUrl}
          {saveStoryContent}
        />
      {:else}
        <CullView
          {view}
          loading={library.loading}
          {sel}
          selectedPaths={selection.paths}
          {storySet}
          {layout}
          {cols}
          {marginScale}
          {cellAspect}
          {fillCells}
          progress={activity.progress}
          bind:currentScrollTop={currentScrollTop}
          curDir={library.curDir}
          {minRating}
          {isTauri}
          {debug}
          selectGridItem={(/** @type {number} */ i, /** @type {MouseEvent | undefined} */ e) =>
            selectGridItem(view, i, e)}
          {openPhoto}
          {openPhotoMenu}
          {toggleStoryWithPath}
          {onPhotoDragStart}
          {closePhotoMenu}
          hasRoot={!!library.root || applePhotosActive}
          {applePhotosActive}
          {scanning}
          onAddLibraryFolder={indexRoot}
          {gridProseByRow}
          onSaveProse={saveGridProse}
        />
      {/if}
      </div>
    </div>
    <ContextMenu
      {photoMenu}
      selectedPaths={selection.paths}
      {installedEditors}
      {copiedRecipe}
      {storySet}
      {gardenUrl}
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
      onDevelopToVault={preferences.obsidian_enabled ? (/** @type {string} */ p) => developFromMenu(p, true) : undefined}
      onCull={applePhotosActive ? undefined : cullCurrentFolder}
      onPublishStory={publishStory}
      onOpenGardenUrl={() => { if (gardenUrl) invoke("open_path", { path: gardenUrl }); }}
    />
    {#if false && layouts[currentMode].focus}
      <div class="focus-overlay" aria-hidden="true"></div>
    {/if}

    {#if catalogOpen}
      <Dialog bind:open={catalogOpen} label="Catalogue note">
          <div class="modal-header">
            <h3>CATALOGUE NOTE (REVEAL.MD)</h3>
            <button class="close-btn" onclick={() => (catalogOpen = false)} aria-label="Close catalogue note">✕</button>
          </div>
          <div class="modal-body">
            <textarea
              bind:value={catalogContent}
              aria-label="Catalogue note"
              placeholder="Write global notes for this library…"
            ></textarea>
          </div>
          <div class="modal-footer">
            <button onclick={async () => { await saveCatalogNote(); catalogOpen = false; }}>Save</button>
            <button class="secondary" onclick={() => (catalogOpen = false)}>Close</button>
          </div>
      </Dialog>
    {/if}
  </div>
{:else}
  {@const showDockedPanel = isTauri && recipe && layouts.dev.devPanel && !layouts.dev.detached}
  <div
    class="app"
    class:themed={appThemed}
    role="presentation"
    style="grid-template-columns: {showDockedPanel ? '1fr 17rem' : (layouts.dev.devPanel && !isTauri) ? '1fr 19.5rem' : '1fr'};"
    style:--color-background={appBg}
    style:--color-foreground={appFg}
    style:--color-accent={appAccent}
    style:--font-header={appFontHeader}
    style:--font-text={appFontText}
    style:--font-ratio={appFontRatio}
    style:--radius={appMeasuredRadius["--radius"]}
    style:--radius-sm={appMeasuredRadius["--radius-sm"]}
    style:--radius-lg={appMeasuredRadius["--radius-lg"]}
    onmousedown={startWindowDrag}
  >
    <DevelopView
      {picked}
      imgUrl={imgUrl ?? undefined}
      {useCanvas}
      {canvasVersion}
      {showClipping}
      {caption}
      {showCaption}
      {recipe}
      {renderAspect}
      bind:canvasEl={canvasEl}
      bind:imgFailed={imgFailed}
      bind:histogram
      bind:scopes
      {status}
      {inflight}
      {pendingPx}
      {zoomMode}
      {panning}
      {developPhotoPercent}
      {sourceOffline}
      {onPhotoPointerDown}
      {onPhotoPointerMove}
      {onPhotoPointerUp}
      showCropOverlay={!!showDockedPanel && dockedActiveTab === "crop"}
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
                <input type="checkbox" role="switch" bind:checked={recipe.auto_exposure} onchange={() => edited()} />
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
            placeholder="caption…"
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
            <input type="checkbox" role="switch" bind:checked={exportBorder} />
          </label>
          <button onclick={() => exportCurrent()}>Exporter cette photo</button>
        </section>

        <!-- Section: Tone -->
        <section class="collapsible">
          <button class="section-toggle" onclick={() => tonalityOpen = !tonalityOpen}>
            <span>TONE</span>
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
                <span>HL RECOVERY</span>
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
                <span>SHARPNESS</span>
                <input type="range" min="0" max="1" step="0.05" bind:value={recipe.sharpen} style="--f: {pct(recipe.sharpen, 0, 1)}" oninput={() => edited(true)} onchange={() => edited(false)} />
                <code>{fmt(recipe.sharpen)}</code>
              </label>
            </div>
          {/if}
        </section>
      </aside>
    {:else if showDockedPanel}
      <div class="docked-panel-frame">
      <DevelopPanel
        {photoPath}
        {picked}
        bind:recipe
        bind:activeTab={dockedActiveTab}
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
        rating={currentRating}
        bind:publishing={devPublishing}
        bind:publishStatus={devPublishStatus}
        {histogram}
        {scopes}
        bind:photoScale={developPhotoPercent}
        {showClipping}
        toggleClipping={dockedToggleClipping}
        {showCaption}
        toggleCaptionOverlay={dockedToggleCaptionOverlay}
        edited={dockedEdited}
        {resetOne}
        {addLutLayer}
        {removeLutLayer}
        {updateLutOpacity}
        {setLutFile}
        engineChanged={dockedEngineChanged}
        resetRecipe={applyResetRecipe}
        hidePanel={dockedHidePanel}
        onCaptionEdited={captionEdited}
        onTagsEdited={tagsEdited}
        onExportSettingsChanged={dockedExportSettingsChanged}
        onExport={exportCurrent}
        onExportDaily={exportToDailyNote}
        onChooseExportFolder={chooseExportFolder}
        onOpenInEditor={openInEditor}
        detached={false}
        onToggleDetached={() => {
          layouts.dev.detached = true;
          saveLayouts();
        }}
      />
      </div>
    {/if}
  </div>
{/if}
<a href="/dev-panel" style="display: none;">Prerender Target</a>
<a href="/settings-panel" style="display: none;">Prerender Target</a>

<!-- The one place every long-running operation reports to, regardless of
     which mode (Grid/Develop) is currently showing — an import can finish
     while you're in Develop, and you should still see it. -->
<NotificationStack>
  <Toast message={activity.message} />
  <TaskIndicator activityQueue={activity.queue} onOpen={() => setQueueOpen(true)} />
</NotificationStack>
{#if activity.queueOpen}
  <RenderQueueModal
    activityQueue={activity.queue}
    activeActivityId={activity.activeId}
    onClose={() => setQueueOpen(false)}
    onCancelQueue={cancelExportQueue}
  />
{/if}

{#if fullscreen && view[sel]}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="fullscreen-photo" onclick={exitFullscreen} role="presentation">
    <!-- The real single-photo viewer, so fullscreen is identical to dev mode
         — including the same developPhotoPercent (the size slider in DevTab),
         so a photo sized to overflow its dev-panel frame overflows here too.
         Fullscreen just shows the pre-developed `fullscreenUrl`. -->
    <DevelopView
      picked={view[sel].name}
      imgUrl={fullscreenUrl ?? undefined}
      useCanvas={false}
      zoomMode="frame"
      {developPhotoPercent}
      {sourceOffline}
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
  .photos-transfer {
    position: fixed;
    bottom: var(--space);
    left: 50%;
    transform: translateX(-50%);
    z-index: 10000;
    display: flex;
    align-items: center;
    gap: var(--space);
    max-width: 80vw;
    font-family: var(--font-monospace);
    font-size: 12px;
  }
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
  /* Folder mood (.cull and .app, dev mode further below) — re-derives the
     surface/border scale from the SAME --color-background/--color-foreground
     the framework's own dark-mode block computes them from
     (packages/styles/_standard-02-color.scss), just off our per-library.folder
     override instead of the theme default. Gated behind .themed so an
     unthemed library.folder's surfaces stay byte-identical to before this existed —
     only folders with an actual story-theme note shift.
     `background`/`color` are painted here explicitly: nothing under .cull/
     .app actually draws with --color-background itself (only the outer
     `html` does, per _standard-07-base.scss) — .cull/.app were always
     transparent, relying on html's single background layer showing through.
     Redefining the CSS VARIABLE alone on a transparent descendant changes
     nothing visible; only html (an ANCESTOR, unreachable from here since
     custom properties don't inherit upward) painted with it. */
  .cull.themed,
  .app.themed {
    --color-surface: color-mix(in srgb, var(--color-foreground) 6%, var(--color-background));
    --color-border: color-mix(in srgb, var(--color-foreground) 14%, transparent);
    --color-surface-low: color-mix(in srgb, black 3%, var(--color-surface));
    --color-surface-lowest: color-mix(in srgb, black 7%, var(--color-surface-low));
    --color-surface-high: color-mix(in srgb, var(--color-foreground) 3%, var(--color-surface));
    --color-surface-highest: color-mix(in srgb, var(--color-foreground) 3%, var(--color-surface-high));
    background: var(--color-background);
    color: var(--color-foreground);
  }
  .window-controls-zone {
    position: fixed;
    top: 0;
    left: 0;
    padding: 15px 18px 24px 18px;
    z-index: 100;
    display: inline-flex;
  }
  .window-controls-zone.dev {
    z-index: 1001;
  }
  .window-controls-zone.fullscreen {
    display: none !important;
  }
  .window-controls {
    display: flex;
    gap: 8px;
    transition: opacity var(--duration-standard) var(--ease-soft);
  }
  /* In develop mode: hide traffic lights unless hovering the corner area */
  .window-controls.dev {
    opacity: 0;
    pointer-events: none;
  }
  .window-controls-zone:hover .window-controls.dev {
    opacity: 1;
    pointer-events: auto;
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
  @keyframes fullscreen-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .fullscreen-photo {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-background, #0e0e0e);
    cursor: zoom-out;
    animation: fullscreen-fade-in 180ms cubic-bezier(0.16, 1, 0.3, 1) forwards;
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

  .rail :global(.rail-btn) {
    all: unset;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .rail :global(.rail-btn:hover) {
    color: var(--color-foreground);
  }
  .rail :global(.rail-btn.star-filter-btn) {
    width: auto;
    gap: 3px;
    padding: 1px 4px;
    border-radius: var(--radius-sm);
  }
  .rail-badge {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    font-weight: 600;
    line-height: 1;
    padding: 1px 3.5px;
    background: color-mix(in srgb, var(--color-foreground) 15%, transparent);
    color: var(--color-foreground);
    border-radius: 3px;
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

  /* Only photo-grid geometry controls remain app-owned; Popover owns the shell. */
  .layout-controls {
    min-width: 208px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .pop-divider {
    height: 1px;
    background: var(--color-border);
    margin: 4px 0;
  }
  /* A menu row that cannot be chosen, and the count that says why. Dimmed
     rather than hidden: the option still belongs in the list, it just is not
     available for this folder. */
  .std-menu-item[disabled] {
    opacity: 0.45;
    cursor: default;
    pointer-events: auto; /* keep the title tooltip reachable */
  }
  .item-note {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    white-space: nowrap;
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

  /* ================= develop ================= */
  .app {
    display: grid;
    grid-template-columns: 1fr 19.5rem;
    height: 100vh;
    position: relative;
    z-index: 1;
    /* body's own background-image (a --color-surface-low wash, see
       +layout.svelte) is lighter than DevelopView's <main>, which paints
       --color-background over its own column — leaving the docked panel's
       margin gutter, uncovered by either, showing that lighter body tone.
       Paint the whole grid one flat shade so both columns' gutters match. */
    background: var(--color-background);
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
    background: var(--color-surface-high);
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
  /* Same floating-card treatment as the real sidebar (modules/sidebar/
     Sidebar.svelte's `nav`: margin 8px, --radius-lg, that exact shadow) —
     DevelopPanel's own .panel is edge-to-edge on purpose (it also fills a
     whole DETACHED OS window, where the window chrome itself already
     supplies the rounding), so docking it inline needs this wrapper to
     match rather than sitting flush against the window edge. */
  .docked-panel-frame {
    /* No `height: 100%` here. As a grid item it already stretches to the
       row, and that stretch subtracts these margins; `height: 100%` would
       resolve against the full track and then ADD them, pushing the panel
       16px past the bottom of the window (Francis, 2026-09-22: "il descend
       plus bas que l'app"). */
    margin: 8px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.22);
    overflow: hidden;
    min-height: 0;
    /* Matches nav's own self-painted background (Sidebar.svelte) exactly —
       relying on DevelopPanel's child .panel to fill this instead left the
       frame's own edges reading with none of nav's "floating card" look. */
    background: var(--color-surface-high);
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

  /* Catalogue-note content; Dialog supplies the shared modal shell. */
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
  .queue-activity.progress-bar {
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
