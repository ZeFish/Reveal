/**
 * @typedef {Object} ControllerState
 * @property {string} currentMode
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
    if (this.state.currentMode === "dev") {
      // If we are in single view (quick look or dev mode), return to grid
      return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
    } else {
      // If we are in grid, open quick single-photo view (spaceLook)
      return { action: "SWITCH_MODE", to: "dev", spaceLook: true, openDevPanel: false };
    }
  }

  handleD() {
    if (this.state.currentMode === "dev") {
      if (this.state.spaceLook || !this.state.devPanel) {
        // If we are in quick look or panel is closed, just open the panel (stay in dev mode)
        return { action: "OPEN_DEV_PANEL", spaceLook: false };
      } else {
        // If we are already in full dev mode with panel open, toggle back to grid
        return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
      }
    } else {
      // If we are in grid (or story), jump to full dev mode with panel
      return { action: "SWITCH_MODE", to: "dev", spaceLook: false, openDevPanel: true };
    }
  }

  handleG() {
    if (this.state.currentMode === "cull") {
      return { action: "TOGGLE_SIDEBAR" };
    }
    return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
  }

  handleS() {
    if (this.state.currentMode === "story") {
      return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
    }
    return { action: "SWITCH_MODE", to: "story", spaceLook: false };
  }

  /**
   * @param {{ hasOverlay?: boolean, fullscreen?: boolean }} args
   */
  handleEscape({ hasOverlay, fullscreen }) {
    if (hasOverlay) {
      return { action: "CLOSE_OVERLAY" };
    }
    if (fullscreen) {
      return { action: "EXIT_FULLSCREEN" };
    }
    if (this.state.currentMode !== "cull") {
      return { action: "SWITCH_MODE", to: "cull", spaceLook: false };
    }
    return { action: "NONE" };
  }
}
