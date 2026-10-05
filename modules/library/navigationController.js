/**
 * Navigation controller coordinating catalogue directory and folder opening.
 */

import { pickFolder as opPickFolder } from "./libraryOperations.js";

/**
 * Creates a bound folder navigation controller.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   session: any,
 *   cullingState: { minRating: number, layout: string, activeVirtualCollectionId?: string | null },
 *   storyState: { filterStory: boolean },
 *   beginOpen: (opts: { curDir?: string, folder?: string }) => any,
 *   withPreviewVersions: (rows: any[]) => Promise<any[]>,
 *   openFolderSession: (path: string) => any,
 *   scrollOffsets: Record<string, number>,
 *   switchMode: (mode: "dev" | "cull") => Promise<void> | void,
 *   refreshStory: () => Promise<void> | void,
 *   focusAt: (view: any[], index: number) => void,
 *   selectOnly: (view: any[], index: number) => void,
 *   openPhoto: (path: string, opts?: { openDevPanel?: boolean }) => Promise<void> | void,
 *   layouts: any,
 *   getView: () => any[],
 *   leaveApplePhotos: () => void,
 *   leaveImmich: () => void,
 *   handleOpenApplePhotos: (subpath: string, opts?: { authorize?: boolean }) => Promise<void>,
 *   handleOpenImmich: (subpath: string) => Promise<void>,
 *   setFolderSession: (sess: any) => void,
 *   setCurrentScrollTop: (pos: number) => void,
 *   setPreviewFilter: (val: boolean) => void,
 *   setLoading: (val: boolean) => void,
 *   setCurDir?: (dir: string | null) => void,
 *   setDebug?: (msg: string) => void,
 *   isMasonryTooBig?: () => boolean,
 *   applePhotosRoot?: string,
 *   immichRoot?: string,
 *   log?: (msg: string) => void,
 * }} deps
 */
export function createNavigationController(deps) {
  const {
    invoke,
    session,
    cullingState,
    storyState,
    beginOpen,
    withPreviewVersions,
    openFolderSession,
    scrollOffsets,
    switchMode,
    refreshStory,
    focusAt,
    selectOnly,
    openPhoto,
    layouts,
    getView,
    leaveApplePhotos,
    leaveImmich,
    handleOpenApplePhotos,
    handleOpenImmich,
    setFolderSession,
    setCurrentScrollTop,
    setPreviewFilter,
    setLoading,
    setCurDir = () => {},
    setDebug = () => {},
    isMasonryTooBig = () => false,
    applePhotosRoot = "apple-photos://",
    immichRoot = "immich://",
    log = () => {},
  } = deps;

  /**
   * @param {string} dir
   * @param {boolean} [restoreMode]
   * @param {boolean} [restoreSession]
   * @param {boolean} [keepFilters]
   */
  async function openDir(dir, restoreMode = true, restoreSession = false, keepFilters = false) {
    if (dir?.startsWith(applePhotosRoot)) {
      await handleOpenApplePhotos(dir.slice(applePhotosRoot.length), { authorize: restoreMode });
      return;
    }
    if (dir?.startsWith(immichRoot)) {
      await handleOpenImmich(dir.slice(immichRoot.length));
      return;
    }
    leaveApplePhotos();
    leaveImmich();

    if (dir?.startsWith("virtual://")) {
      if (cullingState) cullingState.activeVirtualCollectionId = dir;
      setCurDir(dir);
      session.setLastDirectory(dir);
      if (!keepFilters) {
        cullingState.minRating = 0;
        storyState.filterStory = false;
        setPreviewFilter(false);
      }
      const sess = openFolderSession(dir);
      setFolderSession(sess);
      setCurrentScrollTop(0);
      const view = getView();
      if (view.length > 0) {
        focusAt(view, 0);
        selectOnly(view, 0);
      }
      await switchMode("cull");
      return;
    }

    if (cullingState) cullingState.activeVirtualCollectionId = null;

    const open = beginOpen({ curDir: dir });
    session.setLastDirectory(dir);

    if (!keepFilters) {
      cullingState.minRating = 0;
      storyState.filterStory = false;
      setPreviewFilter(false);
    }
    setDebug("invoke…");
    log(`openDir start dir=${dir} minRating=${JSON.stringify(cullingState.minRating)}`);

    try {
      const rawRows = await invoke("index_frames", { dir, minRating: cullingState.minRating });
      const rows = (Array.isArray(rawRows) ? rawRows : []).filter(
        (r) => r.name && !r.name.startsWith(".") && !r.name.startsWith("._")
      );
      setDebug(`received ${rows.length}`);
      if (!open.commit(rows)) return;
      if (isMasonryTooBig() && cullingState.layout === "masonry") {
        cullingState.layout = "uniform";
      }
      withPreviewVersions(rows).then((updated) => {
        if (Array.isArray(updated)) open.replace(updated);
      });
    } catch (e) {
      setDebug(`failed: ${e}`);
      log(`openDir failed: ${e}`);
    }

    const sess = openFolderSession(dir);
    setFolderSession(sess);
    setCurrentScrollTop(sess.scroll || scrollOffsets[dir] || 0);

    const view = getView();
    focusAt(view, 0);
    selectOnly(view, 0);

    let restoredToDevelop = false;
    if (restoreSession) {
      const savedPhoto = session.lastPhoto();
      const savedIdx = savedPhoto ? view.findIndex((f) => f.path === savedPhoto) : -1;
      if (savedPhoto && sess.mode === "dev" && savedIdx !== -1) {
        focusAt(view, savedIdx);
        selectOnly(view, savedIdx);
        await openPhoto(savedPhoto, { openDevPanel: layouts?.dev?.devPanel });
        restoredToDevelop = true;
      }
    }
    if (!restoredToDevelop) await switchMode("cull");
    await refreshStory();
    setLoading(false);
  }

  /** @param {string} path */
  async function openFolder(path) {
    leaveApplePhotos();
    const open = beginOpen({ folder: path });

    cullingState.minRating = 0;
    storyState.filterStory = false;
    setPreviewFilter(false);

    const rows = await withPreviewVersions(await invoke("list_dir", { path }));
    if (!open.commit(rows)) return;
    open.finish();

    const view = getView();
    focusAt(view, 0);
    selectOnly(view, 0);

    const sess = openFolderSession(path);
    setFolderSession(sess);
    setCurrentScrollTop(sess.scroll || scrollOffsets[path] || 0);

    const savedMode = sess.mode;
    if (savedMode) {
      await switchMode(savedMode);
    } else {
      await switchMode("cull");
    }
    await refreshStory();
  }

  async function pickFolder() {
    await opPickFolder({ invoke, openFolder });
  }

  return {
    openDir,
    openFolder,
    pickFolder,
  };
}
