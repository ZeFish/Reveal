import { invoke } from "@tauri-apps/api/core";
import { hold, dismiss, selection, setSelection, focusAt, selectOnly, session } from "@modules/core";
import { library, beginOpen, appendFrames } from "@modules/library";
import { IMMICH_ROOT } from "./immichTree.js";
import { reloadPhotoPages, restorePhotoSelection, leaveApplePhotos } from "@modules/apple-photos";

/** @typedef {{ path: string, name: string, rating: number, capture_at?: string | number }} Frame */
/** @typedef {{ id: string, title: string, count: number }} ImmichAlbum */

let connecting = $state(false);
let active = $state(false);
let busy = $state(false);
let loaded = $state(false);
let album = $state("");
/** @type {ImmichAlbum[]} */
let albums = $state([]);
/** @type {number | null} */
let libraryTotal = $state(null);
let total = $state(0);
let offset = $state(0);
let request = 0;

export const immich = {
  get connecting() { return connecting; },
  get active() { return active; },
  get busy() { return busy || connecting; },
  get loaded() { return loaded; },
  get album() { return album; },
  get albums() { return albums; },
  get libraryTotal() { return libraryTotal; },
  get currentTotal() { return total; },
  get offset() { return offset; },
  get request() { return request; },

  /**
   * Helper that builds the reactive object expected by Sidebar
   * @param {{ immich_url?: string, immich_api_key?: string }} preferences
   */
  library(preferences) {
    return {
      connected: !!(preferences.immich_url && preferences.immich_api_key),
      active,
      busy: busy || connecting,
      loaded,
      album,
      albums,
      total: libraryTotal,
    };
  },
};

/**
 * @param {{ immich_url?: string, immich_api_key?: string }} preferences
 * @param {boolean} [refresh=false]
 */
export async function loadImmichCollections(preferences, refresh = false) {
  if (!preferences.immich_url || !preferences.immich_api_key) return false;
  connecting = true;
  try {
    if (refresh || !loaded) {
      const res = /** @type {any} */ (await invoke("immich_albums"));
      if (res.connected) {
        albums = res.albums || [];
        libraryTotal = res.total ?? 0;
        loaded = true;
      }
    }
    return true;
  } catch (err) {
    hold(`Could not reach Immich: ${err}`);
    return false;
  } finally {
    connecting = false;
  }
}

/**
 * @typedef {Object} OpenImmichOptions
 * @property {{ immich_url?: string, immich_api_key?: string }} preferences
 * @property {boolean} [refresh=false]
 * @property {boolean} [sortDesc=false]
 * @property {() => any[]} [getView]
 * @property {number} [sel=0]
 * @property {number | null} [selectionAnchor=null]
 * @property {string} [currentMode="cull"]
 * @property {string | null} [photoPath=null]
 * @property {() => void} [onResetFilters]
 * @property {(mode: string) => Promise<void> | void} [onSwitchMode]
 */

/**
 * @param {string} [targetAlbum=""]
 * @param {OpenImmichOptions} [options]
 */
export async function openImmich(targetAlbum = "", options) {
  const {
    preferences = {},
    refresh = false,
    sortDesc = false,
    getView = () => [],
    sel = 0,
    selectionAnchor = null,
    currentMode = "cull",
    photoPath = null,
    onResetFilters = () => {},
    onSwitchMode = () => {},
  } = options || {};

  const req = ++request;
  leaveApplePhotos();
  let preserve = active && album === targetAlbum;
  const currentView = getView();
  const previous = {
    selected: new Set(selection.paths),
    focus: currentView[sel]?.path,
    anchor: selectionAnchor != null ? currentView[selectionAnchor]?.path : undefined,
    index: sel,
  };
  const loadedCount = library.frames.length;
  const descending = sortDesc;
  busy = true;
  dismiss();
  try {
    if (!await loadImmichCollections(preferences, refresh) || req !== request) return;
    /** @type {(offset: number) => Promise<{frames: Frame[], total: number, next: number}>} */
    const readPage = (offset) => invoke("immich_list", { album: targetAlbum || null, offset, descending });
    const page = await reloadPhotoPages(
      readPage,
      {
        minimumCount: preserve ? loadedCount : 0,
        retainedPaths: preserve ? [...previous.selected, previous.focus, previous.anchor,
          currentMode === "dev" && photoPath?.startsWith("immich://") ? photoPath : null] : [],
        isCurrent: () => req === request,
      },
    );
    if (!page || req !== request) return;
    album = targetAlbum;
    active = true;
    total = page.total;
    offset = page.next;
    const open = beginOpen({});
    open.commit(page.frames);
    open.finish();
    if (preserve) {
      const restored = restorePhotoSelection(getView(), previous);
      setSelection(restored.selected, getView()[restored.anchor]?.path ?? null);
      focusAt(getView(), restored.index);
    } else {
      onResetFilters();
      focusAt(getView(), 0);
      selectOnly(getView(), 0);
    }
    session.setLastDirectory(IMMICH_ROOT + targetAlbum);
    if (!preserve) await onSwitchMode("cull");
  } catch (error) {
    if (req === request) hold(`Could not open Immich: ${error}`);
  } finally {
    if (req === request) busy = false;
  }
}

/**
 * @param {boolean} [sortDesc=false]
 */
export async function loadMoreImmich(sortDesc = false) {
  if (busy || !active) return;
  const req = request;
  busy = true;
  try {
    const page = /** @type {any} */ (await invoke("immich_list", {
      album: album || null, offset, descending: sortDesc,
    }));
    if (req !== request) return;
    appendFrames(page.frames);
    offset = page.next;
    total = page.total;
  } catch (error) {
    if (req === request) hold(`Could not load more photos from Immich: ${error}`);
  } finally {
    if (req === request) busy = false;
  }
}

export function leaveImmich() {
  request += 1;
  active = false;
  busy = false;
}
