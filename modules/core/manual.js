/**
 * The manual lives on the site (reveal.photos/manual), written beside the app
 * but published with it. These are the app's doors into it.
 */
import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "./tauri.js";

export const MANUAL_URL = "https://reveal.photos/manual/";

/** @param {string} [page] a path under /manual/, e.g. "cull/ai-cull/#what-leaves-your-mac" */
export const manualUrl = (page = "") => `${MANUAL_URL}${page}`;

/** Opens in the default browser from the app; in a plain browser, a new tab. */
export function openManual(page = "") {
  const url = manualUrl(page);
  if (isTauri) return invoke("open_path", { path: url }).catch(() => {});
  window.open(url, "_blank", "noopener");
}

/**
 * The page that documents each tab of the develop panel.
 * @type {Record<string, string>}
 */
export const TAB_PAGES = {
  dev: "develop/controls/",
  crop: "develop/controls/#crop",
  preset: "develop/controls/#presets",
  info: "editorial/stories/",
  export: "develop/export/",
};
