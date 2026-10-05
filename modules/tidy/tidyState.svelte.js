/**
 * Tidy state store using Svelte 5 runes.
 *
 * Encapsulates the target folder to tidy, the calculation progress,
 * and the generated filing plan.
 */

class TidyState {
  /** @type {string | null} */
  dir = $state(null);

  /** @type {any} */
  plan = $state(null);

  /** @type {string} */
  error = $state("");

  /** @type {{ done: number, total: number }} */
  progress = $state({ done: 0, total: 0 });

  isOpen = $derived(Boolean(this.dir));

  /**
   * Opens the tidy dialog for a directory.
   * @param {string} path
   */
  open(path) {
    this.dir = path;
    this.plan = null;
    this.error = "";
    this.progress = { done: 0, total: 0 };
  }

  /**
   * Closes the tidy dialog.
   */
  close() {
    this.dir = null;
    this.plan = null;
    this.error = "";
    this.progress = { done: 0, total: 0 };
  }

  /**
   * Resets all plan state.
   */
  reset() {
    this.close();
  }
}

export const tidyState = new TidyState();
