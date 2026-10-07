<script>
  import Alert from "@stnd/ui/Alert.svelte";
  /**
   * @typedef {Object} Props
   * @property {any[]} [activityQueue]
   * @property {*} [activeActivityId]
   * @property {() => void} [onClose]
   * @property {() => void} [onCancelQueue]
   */

  /** @type {Props} */
  let {
    activityQueue = [],
    activeActivityId = null,
    onClose = () => {},
    onCancelQueue = () => {},
  } = $props();

  // Cancel only exists for export jobs today (Rust's cancel_exports) — other
  // activity kinds (import, cull, publish, move) have no cancel path yet.
  const activeJob = $derived(activityQueue.find((j) => j.id === activeActivityId));
  const canCancelActive = $derived(activeJob?.kind === "export");
</script>

<!-- Floating panel, not a modal: no backdrop, so the grid/develop canvas
     behind it stays fully clickable while an activity is in flight. -->
<div class="queue-panel card" role="dialog" aria-label="Activity">
  <header class="modal-header">
    <h3>ACTIVITY</h3>
    {#if canCancelActive}
      <button class="queue-cancel" onclick={onCancelQueue}>Cancel Queue</button>
    {/if}
    <button class="close-btn" onclick={onClose} aria-label="Close activity panel">✕</button>
  </header>
  <div class="modal-body queue-list">
    {#if activityQueue.length === 0}
      <p class="empty-queue">No activity this session.</p>
    {:else}
      {#each activityQueue.slice().reverse() as item (item.id)}
        <div class="queue-item card" aria-current={item.id === activeActivityId ? "true" : undefined}>
          <div class="queue-item-meta">
            <span class="queue-time">{item.timestamp}</span>
            <span class="queue-name">{item.label}</span>
          </div>
          <progress value={item.done} max={item.total || 1}></progress>
          <div class="queue-status">
            <span>{item.current || item.phase}</span>
            <span>{item.done} / {item.total}</span>
          </div>
          {#if item.status === "failed"}
            <div role="alert"><Alert class="error">{item.phase}</Alert></div>
          {:else}
            <span class="queue-phase">{item.phase}</span>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .queue-panel {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    z-index: 10000;
    background: var(--color-surface-sunken);
    border-radius: var(--radius-lg);
    width: min(90vw, 380px);
    max-height: min(60vh, 420px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: var(--shadow-raised), var(--shadow-lift);
    color: var(--color-foreground);
  }
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--space) calc(var(--space-d4) * 5);
    box-shadow: var(--shadow-border-bottom);
  }
  .modal-header h3 {
    margin: 0;
  }
  .queue-cancel {
    padding: var(--space-d5) calc(var(--space-d4) * 2);
    cursor: pointer;
  }
  .close-btn {
    cursor: pointer;
    opacity: 0.6;
    padding: var(--space-d5);
  }
  .close-btn:hover {
    opacity: 1;
  }
  .modal-body {
    padding: calc(var(--space-d4) * 5);
    overflow-y: auto;
    flex: 1;
  }
  .empty-queue {
    opacity: 0.5;
    text-align: center;
    padding: var(--space-2) 0;
  }
  .queue-list {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 3);
  }
  .queue-item {
    padding: calc(var(--space-d4) * 3);
    background: var(--color-surface-raised);
    box-shadow: var(--shadow-border);
    border-radius: var(--radius);

    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 2);
  }
  .queue-item-meta {
    display: flex;
    justify-content: space-between;
  }
  .queue-time {
    opacity: 0.5;
  }
  .queue-status {
    display: flex;
    justify-content: space-between;
    opacity: 0.7;
  }
</style>
