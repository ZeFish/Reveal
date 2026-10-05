/**
 * Modal operations: catalog note persistence, debouncing, and update checks.
 */

import { modalState } from "./modalState.svelte.js";

/**
 * Loads the catalogue note (reveal.md) from the root library directory.
 * @param {string | null | undefined} root
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any>, state?: typeof modalState }} options
 * @returns {Promise<string>}
 */
export async function loadCatalogNote(root, { invoke, state = modalState }) {
  if (!root) return "";
  try {
    const raw = await invoke("load_catalog_note", { root });
    const content = typeof raw === "string" ? raw : "";
    state.setCatalogContent(content);
    return content;
  } catch (err) {
    console.error("Failed to load catalogue note:", err);
    return "";
  }
}

/**
 * Persists the catalogue note (reveal.md) to the root library directory.
 * @param {string | null | undefined} root
 * @param {string} content
 * @param {{ invoke: (cmd: string, args?: any) => Promise<any> }} options
 * @returns {Promise<void>}
 */
export async function saveCatalogNote(root, content, { invoke }) {
  if (!root) return;
  try {
    await invoke("save_catalog_note", { root, content: content ?? "" });
  } catch (err) {
    console.error("Failed to save catalogue note:", err);
  }
}

/**
 * Creates a debounced handler for saving catalogue notes as the user types.
 * @param {(root: string, content: string) => Promise<void> | void} saveFn
 * @param {number} [delayMs=600]
 * @returns {(root: string, content: string) => void}
 */
export function createCatalogDebouncer(saveFn, delayMs = 600) {
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let timer = undefined;
  return (root, content) => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      saveFn(root, content);
    }, delayMs);
  };
}

/**
 * Initializes the "What's new" release notes modal check and scheduled background update check.
 * @param {Object} options
 * @param {boolean} options.isTauri
 * @param {boolean} [options.isDev=false]
 * @param {() => { version: string, notes: string } | null} options.takeWhatsNew
 * @param {(opts?: { quiet?: boolean }) => Promise<any> | void} [options.checkForUpdate]
 * @param {typeof modalState} [options.state=modalState]
 * @param {number} [options.checkDelayMs=8000]
 * @returns {() => void} Teardown function to clear scheduled timers
 */
export function initWhatsNew({
  isTauri,
  isDev = false,
  takeWhatsNew,
  checkForUpdate,
  state = modalState,
  checkDelayMs = 8000,
}) {
  if (!isTauri) return () => {};
  if (takeWhatsNew) {
    const data = takeWhatsNew();
    if (data) state.setWhatsNew(data);
  }
  if (isDev || !checkForUpdate) return () => {};
  const timer = setTimeout(() => {
    checkForUpdate({ quiet: true });
  }, checkDelayMs);

  return () => clearTimeout(timer);
}
