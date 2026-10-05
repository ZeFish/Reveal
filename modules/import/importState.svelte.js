/**
 * @typedef {Object} Card
 * @property {string} [volume]
 * @property {string} name
 * @property {string} dcim
 * @property {number} raw_count
 */

export const importState = $state({
  /** @type {Card[]} */
  cards: [],
  /** @type {Card | null} */
  importingCard: null,
  /** @type {Card | null} */
  ejectableCard: null,
  ejecting: false,
  autoImport: false,
  /** @type {string | null} */
  importDir: null,
  /** @type {string | null} */
  lastImportedFolder: null,
  /** @type {Map<string, string[]>} */
  importedByFolder: new Map(),
  lastImportFailureAt: 0,
});
