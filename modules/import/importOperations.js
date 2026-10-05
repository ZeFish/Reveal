import { invoke } from "@tauri-apps/api/core";
import { notify, hold, setProgress, activity } from "@modules/core";
import { importState } from "./importState.svelte.js";

/**
 * @typedef {import("./importState.svelte.js").Card} Card
 */

/**
 * Poll available removable cards.
 * @returns {Promise<Card[]>}
 */
export async function pollCards() {
  try {
    const available = /** @type {Card[]} */ (await invoke("find_cards"));
    importState.cards = available;
    return available;
  } catch {
    return importState.cards;
  }
}

/**
 * Set the import destination folder (sidebar context-menu action).
 * @param {string} path
 */
export async function setImportDir(path) {
  try {
    await invoke("set_import_dir", { path });
    const name = path?.split("/").pop() || "(racine)";
    notify(`Import → ${name}`, 2500);
  } catch (error) {
    hold(`Could not set import folder: ${error}`);
  }
}

/**
 * Cancel the currently running card import.
 */
export function stopImport() {
  invoke("cancel_import").catch(() => {});
}

/**
 * Eject a removable card.
 * @param {Card} card
 */
export async function ejectCard(card) {
  if (!card?.volume || importState.ejecting) return;
  importState.ejecting = true;
  try {
    await invoke("eject_card", { volume: card.volume });
    importState.ejectableCard = null;
    notify(`${card.name} ejected · you can remove the card`, 5000);
  } catch (e) {
    notify(`Eject failed: ${typeof e === "string" ? e : String(e)}`, 6000);
  } finally {
    importState.ejecting = false;
    importState.cards = await invoke("find_cards");
  }
}

/**
 * Handle a card detected/mounted event.
 * @param {Card} card
 * @param {{ onAutoImport?: (card: Card) => void }} [options]
 */
export function handleCardMounted(card, { onAutoImport } = {}) {
  notify(`card detected · ${card.name} (${card.raw_count})`, 4000);
  if (importState.autoImport && !activity.progress) {
    onAutoImport?.(card);
  }
}

/**
 * Handle card unmount event to clear stale ejectable card affordance.
 * @param {string | undefined} dcim
 */
export function handleCardUnmounted(dcim) {
  if (importState.ejectableCard && dcim && importState.ejectableCard.dcim === dcim) {
    importState.ejectableCard = null;
  }
}

/**
 * Import photos from a memory card into the archive.
 * @param {Card} card
 * @param {Object} options
 * @param {string | null} options.archive
 * @param {(folder: string) => Promise<any> | any} [options.onFinishedFolder]
 * @param {() => Promise<any> | any} [options.onRefreshDirs]
 */
export async function importCard(card, { archive, onFinishedFolder, onRefreshDirs }) {
  if (!archive) {
    notify("Add a library first: photos are imported into one.", 6000);
    return;
  }
  setProgress({ verb: "import", done: 0, total: card.raw_count, current: "" });
  importState.lastImportedFolder = null;
  importState.importingCard = card;
  importState.ejectableCard = null;
  try {
    const stats = /** @type {any} */ (await invoke("import_card", { dcim: card.dcim, archive }));
    // Reconcile the import folder in place
    await invoke("scan_folder", { path: archive }).catch(() => {});
    await onRefreshDirs?.();
    if (stats?.folders?.length) {
      const lastFolder = stats.folders[stats.folders.length - 1];
      importState.lastImportedFolder = lastFolder;
      await onRefreshDirs?.();
      if (lastFolder && onFinishedFolder) {
        await onFinishedFolder(lastFolder);
      }
    }
    const summary = stats?.cancelled
      ? `Import stopped · ${stats.copied} imported`
      : `${stats?.copied ?? 0} imported · ${stats?.skipped ?? 0} skipped · ${stats?.failed ?? 0} failed`;
    notify(summary, 5000);
    invoke("notify_user", { title: "Reveal — import", body: summary }).catch(() => {});
    if (card.volume) importState.ejectableCard = card;
  } catch (error) {
    if (Date.now() - importState.lastImportFailureAt > 1500) {
      notify(`Import failed: ${error}`, 7000);
    }
  } finally {
    setProgress(null);
    importState.importingCard = null;
    importState.cards = await invoke("find_cards");
  }
}
