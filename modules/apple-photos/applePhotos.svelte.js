import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri, notify, hold, dismiss, selection, setSelection, focusAt, selectOnly, session } from "@modules/core";
import { library, beginOpen, appendFrames, clearFrames } from "@modules/library";
import { APPLE_PHOTOS_ROOT, findPhotoCollection } from "./applePhotosTree.js";
import { reloadPhotoPages, restorePhotoSelection } from "./applePhotosBrowsing.js";

/** @typedef {import('./applePhotosTree.js').PhotoCollection} PhotoCollection */
/** @typedef {import('./applePhotosTree.js').PhotoLibrary} PhotoLibrary */
/** @typedef {{ path: string, name: string, rating: number, capture_at?: string | number }} Frame */

let supported = $state(false);
let active = $state(false);
let busy = $state(false);
let connecting = $state(false);
let loaded = $state(false);
let album = $state("");
/** @type {PhotoCollection[]} */
let albums = $state([]);
/** @type {number | null} */
let libraryTotal = $state(null);
/** @type {Promise<boolean> | null} */
let connection = null;
let total = $state(0);
let offset = $state(0);
let request = 0;
/** @type {{phase: string, name: string, error?: string} | null} */
let transfer = $state(null);

export const applePhotos = {
  get supported() { return supported; },
  get active() { return active; },
  get busy() { return busy || connecting; },
  get loaded() { return loaded; },
  get album() { return album; },
  get albums() { return albums; },
  get total() { return libraryTotal; },
  get currentTotal() { return total; },
  get offset() { return offset; },
  get request() { return request; },
  get transfer() { return transfer; },
  set transfer(v) { transfer = v; },
};

/**
 * Initializes the Apple Photos event listener and checks initial system status.
 * @returns {() => void} Cleanup unlistener function
 */
export function initApplePhotos() {
  if (!isTauri) return () => {};
  invoke("apple_photos_status", { authorize: false })
    .then((result) => {
      supported = /** @type {any} */ (result).supported;
    })
    .catch((error) => {
      notify(`Could not check Apple Photos availability: ${error}`);
    });

  const unlistenPromise = listen("apple-photos-transfer", ({ payload }) => {
    // An error is shown by the transfer alert (with its Dismiss) and by nothing else: a second
    // announcement through `hold` drew over it at the same spot, bottom centre.
    transfer = /** @type {any} */ (payload);
  });

  return () => {
    unlistenPromise.then((stop) => stop());
  };
}

/**
 * Disclosure can connect the catalogue without changing the current grid.
 * Startup restoration checks access but must never prompt for permission.
 * @param {boolean} [authorize=true]
 * @param {boolean} [refresh=false]
 */
export async function loadApplePhotosCollections(authorize = true, refresh = false) {
  if (connection) {
    const connected = await connection;
    if (!connected && authorize) return loadApplePhotosCollections(authorize, refresh);
    return connected;
  }
  connecting = true;
  connection = (async () => {
    const access = /** @type {any} */ (await invoke("apple_photos_status", { authorize }));
    if (!["authorized", "limited"].includes(access.authorization)) {
      albums = [];
      loaded = false;
      libraryTotal = null;
      if (active) clearFrames();
      if (!authorize) return false;
      throw new Error("Allow Reveal in System Settings > Privacy & Security > Photos, then click Apple Photos again.");
    }
    if (refresh || !loaded) {
      const collection = /** @type {any} */ (await invoke("apple_photos_albums"));
      albums = collection.albums;
      libraryTotal = collection.total;
      loaded = true;
    }
    return true;
  })();
  try {
    return await connection;
  } finally {
    connection = null;
    connecting = false;
  }
}

export async function connectApplePhotos() {
  try {
    dismiss();
    await loadApplePhotosCollections();
  } catch (error) {
    hold(`Could not connect Apple Photos: ${error}`);
  }
}

/**
 * @param {(album: string) => Promise<void>} [onOpenActiveAlbum]
 */
export async function refreshApplePhotos(onOpenActiveAlbum) {
  if (busy || connecting) return;
  if (active) {
    if (onOpenActiveAlbum) {
      await onOpenActiveAlbum(album);
    } else {
      await openApplePhotos(album, { refresh: true });
    }
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
 * @typedef {Object} OpenApplePhotosOptions
 * @property {boolean} [authorize=true]
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
 * @param {OpenApplePhotosOptions} [options]
 */
export async function openApplePhotos(targetAlbum = "", options = {}) {
  const {
    authorize = true,
    refresh = false,
    sortDesc = false,
    getView = () => [],
    sel = 0,
    selectionAnchor = null,
    currentMode = "cull",
    photoPath = null,
    onResetFilters = () => {},
    onSwitchMode = () => {},
  } = options;

  const req = ++request;
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
    if (!await loadApplePhotosCollections(authorize, refresh) || req !== request) return;
    if (targetAlbum && !findPhotoCollection(albums, targetAlbum)) {
      targetAlbum = "";
      preserve = false;
    }
    /** @type {(offset: number) => Promise<{frames: Frame[], total: number, next: number}>} */
    const readPage = (offset) => invoke("apple_photos_list", { album: targetAlbum || null, offset, descending });
    const page = await reloadPhotoPages(
      readPage,
      {
        minimumCount: preserve ? loadedCount : 0,
        retainedPaths: preserve ? [...previous.selected, previous.focus, previous.anchor,
          currentMode === "dev" && photoPath?.startsWith("apple-photos://") ? photoPath : null] : [],
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
    session.setLastDirectory(APPLE_PHOTOS_ROOT + targetAlbum);
    if (!preserve) await onSwitchMode("cull");
  } catch (error) {
    if (req === request) hold(`Could not open Apple Photos: ${error}`);
  } finally {
    if (req === request) busy = false;
  }
}

/**
 * @param {boolean} [sortDesc=false]
 */
export async function loadMoreApplePhotos(sortDesc = false) {
  if (busy || !active) return;
  const req = request;
  busy = true;
  try {
    const page = /** @type {any} */ (await invoke("apple_photos_list", {
      album: album || null, offset, descending: sortDesc,
    }));
    if (req !== request) return;
    appendFrames(page.frames);
    offset = page.next;
    total = page.total;
  } catch (error) {
    if (req === request) hold(`Could not load more photos: ${error}`);
  } finally {
    if (req === request) busy = false;
  }
}

export function leaveApplePhotos() {
  request += 1;
  active = false;
  busy = false;
}

export async function cancelApplePhotosTransfer() {
  try {
    await invoke("apple_photos_cancel");
  } catch (error) {
    hold(`Could not cancel photo download: ${error}`);
  }
}
