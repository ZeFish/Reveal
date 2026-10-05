<script>
  import Dialog from "@stnd/ui/Dialog.svelte";
  import { ManualLink } from "@modules/core";
  /**
   * @typedef {Object} Props
   * @property {() => void} [onClose]
   */

  /** @type {Props} */
  let { onClose = () => {} } = $props();
</script>

<Dialog open label="Keyboard shortcuts" onclose={onClose} style="--dialog-width: min(90vw, 760px); --dialog-max-height: 85vh">
    <header class="panel-header">
      <h2>KEYBOARD SHORTCUTS</h2>
      <button class="close-btn" onclick={onClose} aria-label="Close keyboard shortcuts">✕</button>
    </header>
    <div class="body shortcuts-grid">
      <div class="shortcut-group">
        <h3>NAVIGATION</h3>
        <div class="shortcut-row"><kbd>←</kbd> <kbd>→</kbd> <kbd>↑</kbd> <kbd>↓</kbd> <span>Navigate the grid</span></div>
        <div class="shortcut-row"><kbd>Space</kbd> <span>Preview — single photo ↔ grid</span></div>
        <div class="shortcut-row"><kbd>g</kbd> <span>Back to grid</span></div>
        <div class="shortcut-row"><kbd>m</kbd> <span>Masonry ↔ grid (Cull) / Zone mask (Dev)</span></div>
        <div class="shortcut-row"><kbd>f</kbd> <span>Full screen</span></div>
        <div class="shortcut-row"><kbd>l</kbd> <span>Toggle light / dark (system)</span></div>
        <div class="shortcut-row"><kbd>+</kbd> <span>Zoom in: photo size (Dev) / fewer photos (Grid)</span></div>
        <div class="shortcut-row"><kbd>−</kbd> <span>Zoom out: photo size (Dev) / more photos (Grid)</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> <kbd>0</kbd> <span>Reset photo size (Dev)</span></div>
        <div class="shortcut-row"><kbd>⇧</kbd> <kbd>+</kbd> / <kbd>⇧</kbd> <kbd>−</kbd> <span>Grid margin (Grid)</span></div>
      </div>
      <div class="shortcut-group">
        <h3>RATING & SELECTION</h3>
        <div class="shortcut-row"><kbd>1</kbd> … <kbd>5</kbd> <span>Rate the photo</span></div>
        <div class="shortcut-row"><kbd>0</kbd> <span>Clear the rating</span></div>
        <div class="shortcut-row"><kbd>⇧</kbd> click / arrows <span>Continuous multi-select</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> click / Space <span>Isolated multi-select</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> <kbd>A</kbd> <span>Select all</span></div>
        <div class="shortcut-row"><kbd>⌘</kbd> <kbd>D</kbd> <span>Deselect all</span></div>
        <div class="shortcut-row"><kbd>q</kbd> <span>Add / remove from the quick collection</span></div>
      </div>
      <div class="shortcut-group">
        <h3>PANELS</h3>
        <div class="shortcut-row"><kbd>d</kbd> <span>Develop</span></div>
        <div class="shortcut-row"><kbd>⇧</kbd> <kbd>D</kbd> <span>Develop panel</span></div>
        <div class="shortcut-row"><kbd>z</kbd> / <kbd>⇧</kbd> <kbd>z</kbd> <span>Zoom: frame → fit → 100% (⇧ reverses)</span></div>
        <div class="shortcut-row"><kbd>s</kbd> <span>Editorial (filter on the grid)</span></div>
        <div class="shortcut-row"><kbd>b</kbd> <span>Folder panel</span></div>
        <div class="shortcut-row"><kbd>o</kbd> <span>Focus mode</span></div>
        <div class="shortcut-row"><kbd>Esc</kbd> <span>Back to grid</span></div>
      </div>
      <div class="shortcut-group">
        <h3>SETTINGS & ACTION</h3>
        <div class="shortcut-row"><kbd>c</kbd> <span>Copy settings</span></div>
        <div class="shortcut-row"><kbd>v</kbd> <span>Paste settings (selection)</span></div>
        <div class="shortcut-row"><kbd>r</kbd> <span>Reveal — develops + exports the selection</span></div>
        <div class="shortcut-row"><kbd>?</kbd> / <kbd>h</kbd> <span>Show this help</span></div>
      </div>
    </div>
    <footer class="panel-footer">
      <ManualLink page="start/the-window/" label="The window, in the manual" />
    </footer>
</Dialog>

<style>
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space) calc(var(--space-d4) * 5);
    border-bottom: var(--stroke-width) solid var(--color-border);
  }
  .panel-header h2 {
    margin: 0;
  }
  .close-btn {
    cursor: pointer;
    opacity: 0.6;
    padding: var(--space-d5);
  }
  .close-btn:hover {
    opacity: 1;
  }
  .panel-footer {
    display: flex;
    justify-content: flex-end;
    padding: var(--space-d2) calc(var(--space-d4) * 5);
    border-top: var(--stroke-width) solid var(--color-border);
  }
  .shortcuts-grid {
    padding: calc(var(--space-d4) * 5);
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: calc(var(--space-d4) * 5);
  }
  .shortcut-group h3 {
    margin: 0 0 calc(var(--space-d4) * 2) 0;
  }
  .shortcut-row {
    display: flex;
    align-items: center;
    gap: calc(var(--space-d4) * 2);
    margin-bottom: calc(var(--space-d4) * 2);
  }
  .shortcut-row span {
    margin-left: var(--space-d5);
    opacity: 0.8;
  }
  kbd {
    padding: var(--space-d6) calc(var(--space-d4) * 2);
  }
</style>
