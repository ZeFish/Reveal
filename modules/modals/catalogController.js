/**
 * Catalog note controller.
 *
 * Coordinates loading and debounced persistence of the library root catalogue note (reveal.md).
 */

import { loadCatalogNote, saveCatalogNote, createCatalogDebouncer } from "./modalOperations.js";

/**
 * @param {{
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   modalState: typeof import("./modalState.svelte.js").modalState,
 *   getRoot?: () => string | null | undefined,
 *   delayMs?: number,
 * }} deps
 */
export function createCatalogController({
  invoke = async () => {},
  modalState,
  getRoot = () => null,
  delayMs = 600,
}) {
  const debouncedSave = createCatalogDebouncer((root, content) => {
    saveCatalogNote(root, content, { invoke });
  }, delayMs);

  async function loadCatalog() {
    const root = getRoot();
    if (!root) return;
    await loadCatalogNote(root, { invoke, state: modalState });
  }

  /**
   * @param {string} content
   */
  function catalogEdited(content) {
    modalState.setCatalogContent(content);
    const root = getRoot();
    if (root) debouncedSave(root, content);
  }

  return {
    loadCatalog,
    catalogEdited,
  };
}
