/**
 * There is only one browsing mode: the grid ("cull"). "Develop" is the only
 * other destination — a single photo open for editing. Storytelling used to
 * be a third destination ("story"); it is now `previewFilter`, a display
 * filter you can flip while never leaving the grid — the file on disk and
 * the interactions (drag to reorder, remove, add a paragraph) don't change,
 * only which photos are shown and how they're laid out.
 *
 * @typedef {Object} ControllerState
 * @property {"cull" | "dev"} currentMode
 * @property {boolean} [spaceLook]
 * @property {boolean} [devPanel]
 * @property {boolean} [sidebarOpen]
 */

export class AppController {
  /**
   * @param {ControllerState} state
   */
  constructor(state) {
    this.state = state;
  }

  handleSpace() {
    if (this.isDev()) {
      // If we are in single view (quick look or dev mode), return to grid
      return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
    } else {
      // If we are in grid, open quick single-photo view (spaceLook)
      return { action: "SWITCH_MODE", to: "dev", spaceLook: true, openDevPanel: false };
    }
  }

  handleD() {
    if (this.isDev()) {
      if (this.state.spaceLook || !this.state.devPanel) {
        // If we are in quick look or panel is closed, just open the panel (stay in dev mode)
        return { action: "OPEN_DEV_PANEL", spaceLook: false };
      } else {
        // If we are already in full dev mode with panel open, toggle back to grid
        return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
      }
    } else {
      // If we are in the grid (Preview on or off), jump to full dev mode with panel
      return { action: "SWITCH_MODE", to: "dev", spaceLook: false, openDevPanel: true };
    }
  }

  /**
   * G always means "show me the plain working grid" — toggling the sidebar
   * when you're already there, otherwise collapsing back to it (out of
   * Develop, or dropping the Preview filter).
   * @param {{ previewFilter?: boolean }} args
   */
  handleG({ previewFilter = false } = {}) {
    if (this.isCull() && !previewFilter) {
      return { action: "TOGGLE_SIDEBAR" };
    }
    return { action: "GO_TO_GRID" };
  }

  /**
   * S flips the Preview filter. From Develop it always lands in the grid
   * WITH Preview on — same "S jumps to storytelling" muscle memory as the old
   * mode switch, just expressed as a filter now.
   */
  handleS() {
    if (this.isDev()) {
      return { action: "TOGGLE_PREVIEW", andSwitchToCull: true };
    }
    return { action: "TOGGLE_PREVIEW" };
  }

  /**
   * @param {{ hasOverlay?: boolean, fullscreen?: boolean, previewFilter?: boolean }} args
   */
  handleEscape({ hasOverlay, fullscreen, previewFilter = false }) {
    if (hasOverlay) {
      return { action: "CLOSE_OVERLAY" };
    }
    if (fullscreen) {
      return { action: "EXIT_FULLSCREEN" };
    }
    if (this.isDev()) {
      return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
    }
    if (previewFilter) {
      return { action: "GO_TO_GRID" };
    }
    return { action: "NONE" };
  }

  isCull() {
    return this.state.currentMode === "cull";
  }

  isDev() {
    return this.state.currentMode === "dev";
  }

  /**
   * Copy targets the single photo open in Develop; everywhere else (grid
   * views) it's whatever the cursor is currently on.
   * @param {{ photoPath?: string | null, selectedFramePath?: string | null }} args
   */
  resolveCopyTarget({ photoPath, selectedFramePath }) {
    return this.isDev() ? photoPath : (selectedFramePath || photoPath);
  }

  // Zoom cycling and per-key recipe nudging only mean something while a
  // single RAW is open for editing.
  canCycleZoom() {
    return this.isDev();
  }

  canAdjustRecipe() {
    return this.isDev();
  }

  /**
   * Grid navigation steps one column per ArrowUp/Down; Develop (and
   * fullscreen quick-look) show a single photo, so a "row" is one photo.
   * @param {{ fullscreen?: boolean, cols: number }} args
   */
  navColumnCount({ fullscreen, cols }) {
    return fullscreen || this.isDev() ? 1 : cols;
  }

  // Arrow-key navigation only opens the photo under the cursor in Develop —
  // in the grid it just moves the selection.
  shouldOpenOnNav() {
    return this.isDev();
  }
}
