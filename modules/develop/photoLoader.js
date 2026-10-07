/**
 * Photo Loader & Viewport URL helpers.
 *
 * Coordinates loading raw photos into the develop workspace, sidecar
 * recipe extraction, multi-stage proxy resolution (cache -> 2048 -> RAW render),
 * undo-stack resets, and mode transitions.
 */

import { Photo } from "@modules/core";
import { PREVIEW_PX } from "./developOperations.js";

/**
 * The full-resolution developed sidecar, for single-photo views.
 * @param {string} path
 * @param {number} [version]
 * @param {boolean} [asShot] the camera's own picture, whatever develop settings the photo carries
 */
export function previewUrl(path, version = 0, asShot = false) {
  return Photo.thumb(path, { version, size: 2048, priority: true, asShot });
}

/**
 * The grid-size copy for a single-photo view.
 * @param {string} path
 * @param {number} [version]
 */
export function openingUrl(path, version = 0) {
  return Photo.thumb(path, { version });
}

/**
 * Creates an openPhoto function bound to the active develop and viewer state.
 *
 * @param {{
 *   invoke: (cmd: string, args?: any) => Promise<any>,
 *   developState: any,
 *   getFrames: () => any[],
 *   getPhotoPath: () => string | null,
 *   setPhotoPath: (path: string | null) => void,
 *   setPicked: (picked: string | null) => void,
 *   getImgUrl: () => string | null,
 *   setImgUrl: (url: string | null) => void,
 *   setStatus: (status: string) => void,
 *   getPreferences: () => any,
 *   prefetchNeighbours: (path: string) => void,
 *   scheduleWorkingPark: (path: string) => void,
 *   scheduleRender: (px: number) => void,
 *   switchMode: (mode: "dev" | "cull", opts?: any) => Promise<any> | void,
 *   setSpaceLook?: (val: boolean) => void,
 *   getCurrentMode: () => "dev" | "cull",
 *   notify?: (msg: string, ms?: number) => void,
 *   previewPx?: number,
 *   createImage?: () => any,
 * }} deps
 */
export function createPhotoLoader(deps) {
  const {
    invoke,
    developState,
    getFrames,
    getPhotoPath,
    setPhotoPath,
    setPicked,
    getImgUrl,
    setImgUrl,
    setStatus,
    getPreferences,
    prefetchNeighbours,
    scheduleWorkingPark,
    scheduleRender,
    switchMode,
    setSpaceLook = () => {},
    getCurrentMode,
    notify = () => {},
    previewPx = PREVIEW_PX,
    createImage = () => (typeof Image !== "undefined" ? new Image() : null),
  } = deps;

  /**
   * @param {string} path
   * @param {{ openDevPanel?: boolean }} [opts]
   */
  async function openPhoto(path, { openDevPanel = true } = {}) {
    const curImg = getImgUrl();
    if (curImg?.startsWith("blob:")) URL.revokeObjectURL(curImg);
    setPhotoPath(path);
    setPicked(path.split("/").pop() ?? null);

    const openVersion = getFrames().find((f) => f.path === path)?.previewVersion ?? 0;
    setImgUrl(openingUrl(path, openVersion));
    developState.useCanvas = false;

    const openPath = path;
    queueMicrotask(() => {
      const full = createImage();
      if (!full) return;
      full.onload = () => {
        if (getPhotoPath() === openPath && !developState.useCanvas) {
          setImgUrl(full.src);
        }
      };
      full.src = previewUrl(openPath, openVersion);
    });

    developState.imgFailed = false;
    setStatus("");

    try {
      const preferences = getPreferences() || {};
      const [sidecar, defaults] = await Promise.all([
        invoke("load_sidecar", { path }),
        invoke("default_recipe"),
      ]);
      if (path !== getPhotoPath()) return;

      developState.caption = sidecar?.description ?? "";
      developState.tags = sidecar?.tags ?? [];
      developState.developEngine = sidecar?.engine
        ? sidecar.engine
        : (sidecar?.engine_settings ? "spektra" : (preferences.default_engine || null));
      developState.recipe = {
        ...defaults,
        ...(sidecar?.engine_settings ?? {}),
        ...(!sidecar?.engine && !sidecar?.engine_settings && preferences.default_engine
          ? { engine: preferences.default_engine }
          : {}),
      };

      developState.resetHistory?.();
      prefetchNeighbours(path);
      scheduleWorkingPark(path);

      if (developState.developEngine) {
        scheduleRender(previewPx);
      } else {
        setStatus("");
      }

      if (getCurrentMode() !== "dev") {
        if (!openDevPanel) setSpaceLook(true);
        await switchMode("dev", { openDevPanel });
      }
    } catch (error) {
      setStatus("Could not load photo");
      notify(`Could not open photo: ${error}`, 5000);
    }
  }

  return {
    openPhoto,
    previewUrl,
    openingUrl,
  };
}
