<script>
  import { Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {boolean} [focusOn]
   * @property {() => void} [onToggleSidebar]
   * @property {() => void} [onToggleFocus]
   * @property {() => void} [onToggleAppearance]
   * @property {() => void} [onShowShortcuts]
   */

  /** @type {Props} */
  let {
    focusOn = false,
    onToggleSidebar = () => {},
    onToggleFocus = () => {},
    onToggleAppearance = () => {},
    onShowShortcuts = () => {},
  } = $props();
</script>

<div class="brand" data-tauri-drag-region>
  <button class="ghost icon small" onclick={onToggleSidebar} title="Folder panel (B)">
    <Icon name="sidebar-simple" size="var(--icon-md)" />
  </button>
  <button
    class="ghost icon small"
    aria-pressed={focusOn}
    onclick={onToggleFocus}
    title="Focus mode (O)"
  >
    <span class="focus-glyph" class:on={focusOn}></span>
  </button>
  <button class="ghost icon small" onclick={onToggleAppearance} title="Toggle system light/dark appearance (L)">
    <Icon name="circle-half" size="var(--icon-md)" />
  </button>
  <button class="wordmark" onclick={onShowShortcuts} title="Keyboard shortcuts">REVEAL</button>
</div>

<style>
  .brand {
    /* The title-bar band, measured from the WINDOW's top edge: the negative
       margin cancels the card's own inset, so these controls centre on the
       traffic lights and on the rail beside them. */
    height: var(--titlebar-height);
    margin-top: calc(var(--window-inset) * -1.25);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0;
    /* Native traffic lights overlay the window top-left — start past them. */
    padding: 0 var(--space) 9px var(--window-controls-offset-sidebar);
  }
  /* circle.circle — a ring with a centred dot, accent when focus is on. */
  .focus-glyph {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 var(--stroke-width) currentColor;
    position: relative;
  }
  .focus-glyph::after {
    content: "";
    position: absolute;
    inset: 2px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 var(--stroke-width) currentColor;
  }
  .focus-glyph.on::after {
    background: currentColor;
  }
  /* A 1em line box, NOT the cap trim the rest of the title bar uses: the
     theme's header font declares a cap height larger than its drawn caps,
     so trimming to it put the wordmark 1.5pt high. */
  .wordmark {
    cursor: pointer;
  }
</style>
