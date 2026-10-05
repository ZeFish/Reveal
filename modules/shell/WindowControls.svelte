<script>
  import { invoke } from "@tauri-apps/api/core";

  /**
   * @typedef {Object} Props
   * @property {string} [currentMode]
   * @property {boolean} [fullscreen]
   * @property {boolean} [isTauri]
   * @property {() => void} [onClose]
   * @property {() => void} [onMinimize]
   * @property {() => void} [onZoom]
   */

  /** @type {Props} */
  let {
    currentMode = "cull",
    fullscreen = false,
    isTauri = false,
    onClose = () => {
      invoke("close_main_window").catch(() => {});
    },
    onMinimize = () => {
      invoke("minimize_main_window").catch(() => {});
    },
    onZoom = () => {
      invoke("zoom_main_window").catch(() => {});
    },
  } = $props();
</script>

{#if isTauri && !fullscreen}
  <div
    class="window-controls-zone"
    class:dev={currentMode === "dev"}
    data-reveal-host
  >
    <!-- In Develop the lights hide until the pointer reaches the corner. -->
    <div
      class="window-controls"
      data-reveal={currentMode === "dev" ? "" : undefined}
      aria-label="Window controls"
    >
      <button class="window-close" onclick={onClose} aria-label="Close window"></button>
      <button class="window-minimize" onclick={onMinimize} aria-label="Minimize window"></button>
      <button class="window-zoom" onclick={onZoom} aria-label="Zoom window"></button>
    </div>
  </div>
{/if}

<style>
  .window-controls-zone {
    position: fixed;
    top: 0;
    left: 0;
    padding: var(--space) var(--space) calc(var(--space-d4) * 6) var(--space);
    z-index: 100;
    display: inline-flex;
  }
  .window-controls-zone.dev {
    z-index: 1001;
  }
</style>
