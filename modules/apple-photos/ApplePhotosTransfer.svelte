<script>
  import Alert from "@stnd/ui/Alert.svelte";

  /**
   * @typedef {Object} Props
   * @property {{ phase: string, name?: string, error?: string } | null} [transfer]
   * @property {() => void | Promise<void>} [onCancel]
   * @property {() => void} [onDismiss]
   */

  /** @type {Props} */
  let {
    transfer = null,
    onCancel = () => {},
    onDismiss = () => {},
  } = $props();
</script>

{#if transfer?.phase === "loading"}
  <div class="photos-transfer" role="status">
    <Alert class="info" title="Apple Photos">
      <span>Preparing {transfer.name} from Apple Photos (iCloud if needed)...</span>
      <button class="btn ghost" onclick={onCancel}>Cancel download</button>
    </Alert>
  </div>
{:else if transfer?.phase === "error"}
  <div class="photos-transfer" role="alert">
    <Alert class="error" title="Apple Photos">
      <span>{transfer.error}</span>
      <button class="btn ghost" onclick={onDismiss}>Dismiss</button>
    </Alert>
  </div>
{/if}

<style>
  .photos-transfer {
    position: fixed;
    bottom: var(--space);
    left: 50%;
    transform: translateX(-50%);
    z-index: 10000;
    display: flex;
    align-items: center;
    gap: var(--space);
    max-width: 80vw;
  }
</style>
