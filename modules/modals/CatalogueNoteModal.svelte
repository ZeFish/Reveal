<script>
  import Dialog from "@stnd/ui/Dialog.svelte";

  /**
   * @typedef {Object} Props
   * @property {boolean} [open]
   * @property {string} [content]
   * @property {() => void | Promise<void>} [onSave]
   * @property {() => void} [onClose]
   */

  /** @type {Props} */
  let {
    open = $bindable(false),
    content = $bindable(""),
    onSave = () => {},
    onClose = () => {},
  } = $props();
</script>

{#if open}
  <Dialog bind:open label="Catalogue note">
    <header class="modal-header">
      <h3>CATALOGUE NOTE (REVEAL.MD)</h3>
      <button class="close-btn" onclick={onClose} aria-label="Close catalogue note">✕</button>
    </header>
    <div class="body modal-body">
      <textarea
        bind:value={content}
        aria-label="Catalogue note"
        placeholder="Write global notes for this library…"
      ></textarea>
    </div>
    <footer class="modal-footer">
      <button onclick={async () => { await onSave(); onClose(); }}>Save</button>
      <button class="accent" onclick={onClose}>Close</button>
    </footer>
  </Dialog>
{/if}

<style>
  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space) calc(var(--space-d4) * 6);
    border-bottom: var(--stroke-width) solid var(--color-border);
    background: var(--color-surface-sunken);
  }
  .modal-header h3 {
    margin: 0;
  }
  .close-btn {
    cursor: pointer;
    opacity: 0.6;
  }
  .close-btn:hover {
    opacity: 1;
  }
  .modal-body {
    padding: calc(var(--space-d4) * 6);
    display: flex;
    flex-direction: column;
    gap: var(--space);
    max-height: 24rem;
    overflow-y: auto;
  }
  .modal-body textarea {
    width: 100%;
    height: 15rem;
    padding: var(--space);
    resize: none;
    font-family: inherit;
    font-size: 13px;
    line-height: 1.5;
    background: transparent;
    color: inherit;
    border: none;
    outline: none;
  }
  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space);
    padding: var(--space) calc(var(--space-d4) * 6);
    border-top: var(--stroke-width) solid var(--color-border);
    background: var(--color-surface-sunken);
  }
  .modal-footer button {
    padding: calc(var(--space-d4) * 2) calc(var(--space-d4) * 5);
    cursor: pointer;
  }
</style>
