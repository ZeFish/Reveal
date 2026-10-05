/**
 * Import workflow controller.
 *
 * Coordinates removable card polling, card imports, card ejection,
 * and auto-import settings toggling.
 */

import {
  pollCards,
  setImportDir,
  stopImport,
  ejectCard,
  handleCardMounted,
  handleCardUnmounted,
  importCard as opImportCard,
} from "./importOperations.js";

/**
 * @param {{
 *   invoke?: (cmd: string, args?: any) => Promise<any>,
 *   notify?: (msg: string, duration?: number) => void,
 *   importState: typeof import("./importState.svelte.js").importState,
 *   getArchiveDir?: () => string | null,
 *   refreshDirs?: () => Promise<any>,
 *   openDir?: (dir: string) => Promise<any>,
 * }} deps
 */
export function createImportController({
  invoke = async () => {},
  notify = () => {},
  importState,
  getArchiveDir = () => null,
  refreshDirs = async () => {},
  openDir = async () => {},
}) {
  async function toggleAutoImport() {
    try {
      const prefs = await invoke("toggle_auto_import");
      importState.autoImport = Boolean(prefs?.auto_import);
      notify(importState.autoImport ? "auto-import enabled" : "auto-import disabled", 2500);
    } catch (e) {
      notify(`Could not toggle auto-import: ${e}`, 3000);
    }
  }

  /**
   * @param {import("./importState.svelte.js").Card} card
   */
  function importCard(card) {
    const archive = importState.importDir ?? getArchiveDir();
    return opImportCard(card, {
      archive,
      onFinishedFolder: async (folder) => {
        await refreshDirs();
        await openDir(folder);
      },
      onRefreshDirs: () => refreshDirs(),
    });
  }

  return {
    toggleAutoImport,
    importCard,
    pollCards,
    setImportDir,
    stopImport,
    ejectCard,
    handleCardMounted,
    handleCardUnmounted,
  };
}
