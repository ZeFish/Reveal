<script>
  // Shown once, on the first launch after an update — the other half of the
  // update card. The notes were stored just before the relaunch.
  import Dialog from "@stnd/ui/Dialog.svelte";
  import { parseNotes } from "./updater.svelte.js";

  /**
   * @typedef {Object} Props
   * @property {string} version
   * @property {string} notes
   * @property {() => void} [onClose]
   */

  /** @type {Props} */
  let { version, notes, onClose = () => {} } = $props();
  const groups = $derived(parseNotes(notes));
</script>

<Dialog open label="What's new" onclose={onClose} style="--dialog-width: min(90vw, 520px); --dialog-max-height: 80vh">
  <header class="panel-header">
    <h2>REVEAL {version}</h2>
    <button class="close-btn" onclick={onClose} aria-label="Close what's new">✕</button>
  </header>
  <div class="body">
    {#each groups as group}
      {#if group.heading}<h3>{group.heading.toUpperCase()}</h3>{/if}
      <ul>
        {#each group.items as item}<li>{item}</li>{/each}
      </ul>
    {:else}
      <p>Reveal is up to date. This release was behind the scenes.</p>
    {/each}
  </div>
  <footer class="panel-footer">
    <button type="button" onclick={onClose}>Continue</button>
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
  .body {
    padding: calc(var(--space-d4) * 5);
    overflow-y: auto;
  }
  .body h3 {
    margin: 0 0 calc(var(--space-d4) * 2);
  }
  .body ul {
    margin: 0 0 calc(var(--space-d4) * 4);
    padding-left: 1.2em;
  }
  .panel-footer {
    display: flex;
    justify-content: flex-end;
    padding: var(--space-d2) calc(var(--space-d4) * 5);
    border-top: var(--stroke-width) solid var(--color-border);
  }
</style>
