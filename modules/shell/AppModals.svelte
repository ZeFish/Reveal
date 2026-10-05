<script>
  import AppModals from "../modals/AppModals.svelte";

  /**
   * @typedef {Object} Props
   * @property {(() => void) | undefined} [onCancelQueue]
   * @property {string | null} [tidyDir]
   * @property {() => void} [onCloseTidy]
   * @property {boolean} [shortcutsOpen]
   * @property {() => void} [onCloseShortcuts]
   * @property {{ version: string, notes: string } | null} [whatsNew]
   * @property {() => void} [onCloseWhatsNew]
   * @property {boolean} [catalogOpen]
   * @property {string} [catalogContent]
   * @property {() => void | Promise<void>} [onSaveCatalog]
   * @property {() => void} [onCloseCatalog]
   */

  /** @type {Props} */
  let { ...props } = $props();
</script>

<AppModals {...props} />
