/**
 * Centralized modal and overlay state for Reveal.
 *
 * Encapsulates the visibility and content of shortcuts cheatsheet,
 * version release notes (What's New), and catalogue notes (reveal.md)
 * using Svelte 5 runes.
 */

/**
 * @typedef {Object} WhatsNewData
 * @property {string} version
 * @property {string} notes
 */

const state = $state({
  /** @type {boolean} */
  shortcutsOpen: false,
  /** @type {WhatsNewData | null} */
  whatsNew: null,
  /** @type {boolean} */
  catalogOpen: false,
  /** @type {string} */
  catalogContent: "",
});

export const modalState = {
  get shortcutsOpen() {
    return state.shortcutsOpen;
  },
  set shortcutsOpen(val) {
    state.shortcutsOpen = !!val;
  },
  get whatsNew() {
    return state.whatsNew;
  },
  set whatsNew(val) {
    state.whatsNew = val;
  },
  get catalogOpen() {
    return state.catalogOpen;
  },
  set catalogOpen(val) {
    state.catalogOpen = !!val;
  },
  get catalogContent() {
    return state.catalogContent;
  },
  set catalogContent(val) {
    state.catalogContent = typeof val === "string" ? val : "";
  },

  openShortcuts() {
    state.shortcutsOpen = true;
  },
  closeShortcuts() {
    state.shortcutsOpen = false;
  },
  toggleShortcuts() {
    state.shortcutsOpen = !state.shortcutsOpen;
  },

  setWhatsNew(/** @type {WhatsNewData | null} */ data) {
    state.whatsNew = data;
  },
  closeWhatsNew() {
    state.whatsNew = null;
  },

  openCatalog() {
    state.catalogOpen = true;
  },
  closeCatalog() {
    state.catalogOpen = false;
  },
  setCatalogContent(/** @type {string} */ content) {
    state.catalogContent = content ?? "";
  },

  reset() {
    state.shortcutsOpen = false;
    state.whatsNew = null;
    state.catalogOpen = false;
    state.catalogContent = "";
  },
};
