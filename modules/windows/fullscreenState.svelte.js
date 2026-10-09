import { invoke } from "@tauri-apps/api/core";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@modules/core";

export const fullscreenState = $state({
  active: false,
  /** @type {string | null} */
  url: null,
  requestId: 0,
});

/**
 * Prepares the frame preview image for fullscreen display.
 * @param {string} path
 * @param {Object} options
 * @param {any} [options.frame]
 * @param {string} [options.currentMode]
 * @param {string | null} [options.photoPath]
 * @param {string | null} [options.imgUrl]
 * @param {any} [options.recipe]
 * @param {(path: string, version: number) => string} options.previewUrl
 * @param {() => string | undefined} [options.getCurrentFramePath]
 */
export async function prepareFullscreenFrame(path, {
  frame,
  currentMode = "cull",
  photoPath = null,
  imgUrl = null,
  recipe = null,
  previewUrl,
  getCurrentFramePath,
}) {
  const request = ++fullscreenState.requestId;
  const immediate =
    currentMode === "dev" && photoPath === path && imgUrl
      ? imgUrl
      : previewUrl(path, frame?.previewVersion ?? 0);
  if (fullscreenState.url?.startsWith("blob:") && fullscreenState.url !== imgUrl) {
    URL.revokeObjectURL(fullscreenState.url);
  }
  fullscreenState.url = immediate;
  if (currentMode === "dev" && photoPath === path && imgUrl?.startsWith("blob:")) return;

  try {
    let settings = recipe;
    if (!settings) {
      const sidecar = /** @type {any} */ (await invoke("load_sidecar", { path }));
      settings = sidecar?.engine_settings;
    }
    if (!settings) return;

    const bytes = /** @type {any} */ (await invoke("develop_preview", {
      path,
      recipe: settings,
      maxPx: 2560,
    }));
    if (!fullscreenState.active || request !== fullscreenState.requestId || getCurrentFramePath?.() !== path) return;
    if (fullscreenState.url?.startsWith("blob:") && fullscreenState.url !== imgUrl) {
      URL.revokeObjectURL(fullscreenState.url);
    }
    fullscreenState.url = URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
  } catch (error) {
    console.error("prepareFullscreenFrame:", error);
  }
}

/**
 * Enter borderless fullscreen mode.
 * @param {Object} options
 * @param {any} options.frame
 * @param {(path: string) => void} [options.prepareFrame]
 */
export async function enterFullscreen({
  frame,
  prepareFrame,
}) {
  if (!frame || fullscreenState.active) return;
  prepareFrame?.(frame.path);
  if (isTauri) {
    const panel = await WebviewWindow.getByLabel("develop-panel");
    if (panel) await panel.hide();
    // Lightroom-style borderless fullscreen (fill display in place)
    await invoke("set_simple_fullscreen", { enabled: true });
    try {
      await getCurrentWindow().setFocus();
    } catch {}
  }
  fullscreenState.active = true;
  if (typeof window !== "undefined") window.focus();
}

/**
 * Exit fullscreen mode.
 * @param {Object} [options]
 * @param {string | null} [options.imgUrl]
 * @param {string} [options.currentMode]
 * @param {any} [options.layouts]
 * @param {() => Promise<void> | void} [options.onSyncDevPanel]
 */
export async function exitFullscreen({
  imgUrl = null,
  currentMode = "cull",
  layouts = null,
  onSyncDevPanel,
} = {}) {
  if (!fullscreenState.active) return;
  fullscreenState.active = false;
  fullscreenState.requestId += 1;
  if (fullscreenState.url?.startsWith("blob:") && fullscreenState.url !== imgUrl) {
    URL.revokeObjectURL(fullscreenState.url);
  }
  fullscreenState.url = null;
  if (isTauri) {
    await invoke("set_simple_fullscreen", { enabled: false });
    try {
      await getCurrentWindow().setFocus();
    } catch {}
    if (currentMode === "dev" && layouts?.dev?.devPanel) {
      await onSyncDevPanel?.();
    }
  }
  if (typeof window !== "undefined") window.focus();
}

/**
 * Toggle fullscreen mode.
 * @param {any} options
 */
export function toggleFullscreen(options) {
  if (fullscreenState.active) return exitFullscreen(options);
  return enterFullscreen(options);
}
