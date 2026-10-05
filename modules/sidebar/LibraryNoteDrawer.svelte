<script>
  import { Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {string} [catalogContent]
   * @property {(value: string) => void} [onCatalogChange]
   * @property {() => void} [onOpenNote]
   */

  /** @type {Props} */
  let {
    catalogContent = "",
    onCatalogChange = () => {},
    onOpenNote = () => {},
  } = $props();

  let libOpen = $state(false);
</script>

<div class="lib-section">
  <button class="lib-toggle" onclick={() => (libOpen = !libOpen)}>
    <span>Library</span>
    <span class="disc" class:open={libOpen}><Icon name="caret-right" size="9px" /></span>
  </button>
  {#if libOpen}
    <textarea
      class="lib-note"
      rows="3"
      placeholder="Catalogue notes…"
      value={catalogContent}
      oninput={(e) => onCatalogChange(e.currentTarget.value)}
    ></textarea>
    <button class="lib-open-note" onclick={onOpenNote}>
      <Icon name="note-pencil" size="10px" />
      <span>Open note</span>
    </button>
  {/if}
</div>

<style>
  .lib-section {
    flex-shrink: 0;
    padding-block: var(--space-d2);
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
  }
  .lib-toggle {
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: transparent;
    border: none;
    padding: 0;
    font: inherit;
    color: inherit;
  }
  .lib-note {
    padding: var(--space-d3);
    resize: vertical;
  }
  .lib-open-note {
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: var(--space-d4);
    background: transparent;
    border: none;
    padding: 0;
    font: inherit;
    color: inherit;
  }
  .disc {
    all: unset;
    cursor: pointer;
    width: 10px;
    height: 14px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: var(--color-muted);
  }
  .disc:hover,
  .disc.open {
    color: var(--color-foreground);
  }
  .disc :global(.icon) {
    transition: all var(--transition-fast);
  }
  .disc.open :global(.icon) {
    transform: rotate(90deg);
  }
</style>
