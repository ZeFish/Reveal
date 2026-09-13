<script>
  import Alert from "@stnd/ui/Alert.svelte";
  /**
   * @typedef {Object} Props
   * @property {any[]} [exportQueue]
   * @property {*} [activeExportJobId]
   * @property {() => void} [onClose]
   * @property {() => void} [onCancelQueue]
   */

  /** @type {Props} */
  let {
    exportQueue = [],
    activeExportJobId = null,
    onClose = () => {},
    onCancelQueue = () => {},
  } = $props();
</script>

<!-- Floating panel, not a modal: no backdrop, so the grid/develop canvas
     behind it stays fully clickable while a render is in flight. -->
<div class="queue-panel" role="dialog" aria-label="Render queue">
  <div class="modal-header">
    <h3>RENDER QUEUE</h3>
    {#if activeExportJobId}
      <button class="queue-cancel" onclick={onCancelQueue}>Cancel Queue</button>
    {/if}
    <button class="close-btn" onclick={onClose} aria-label="Close render queue">✕</button>
  </div>
  <div class="modal-body queue-list">
    {#if exportQueue.length === 0}
      <p class="empty-queue">No renders have run in this session.</p>
    {:else}
      {#each exportQueue.slice().reverse() as item (item.id)}
        <div class="queue-item" class:active={item.id === activeExportJobId}>
          <div class="queue-item-meta">
            <span class="queue-time">{item.timestamp}</span>
            <span class="queue-name">{item.label}</span>
          </div>
          <div class="queue-progress-bar">
            <div class="progress-fill" style="width: {item.total ? (item.done / item.total) * 100 : 0}%"></div>
          </div>
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
    background: var(--color-surface-low, #18181b);
    border: var(--border, 1px solid rgba(255, 255, 255, 0.15));
    border-radius: var(--radius-lg);
    width: min(90vw, 380px);
    max-height: min(60vh, 420px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
    color: var(--color-foreground, #f4f4f5);
  }
  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
  }
  .modal-header h3 {
    font-family: var(--font-header, sans-serif);
    font-size: 0.9rem;
    letter-spacing: 0.1em;
    margin: 0;
  }
  .queue-cancel {
    font-family: var(--font-monospace, monospace);
    font-size: 0.7rem;
    padding: 0.2rem 0.6rem;
    background: rgba(229, 115, 115, 0.2);
    border: 1px solid var(--color-accent);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .queue-cancel:hover {
    background: var(--color-accent);
    color: #fff;
  }
  .close-btn {
    all: unset;
    cursor: pointer;
    opacity: 0.6;
    font-size: 1rem;
    padding: 0.2rem;
  }
  .close-btn:hover {
    opacity: 1;
  }
  .modal-body {
    padding: 1.25rem;
    overflow-y: auto;
    flex: 1;
  }
  .empty-queue {
    font-family: var(--font-text, sans-serif);
    font-size: 0.85rem;
    opacity: 0.5;
    text-align: center;
    padding: 2rem 0;
  }
  .queue-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .queue-item {
    padding: 0.75rem;
    background: var(--color-surface-high, #27272a);
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
    border-radius: var(--radius);

    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .queue-item.active {
    border-color: var(--color-accent);
  }
  .queue-item-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    font-family: var(--font-monospace, monospace);
  }
  .queue-time {
    opacity: 0.5;
  }
  .queue-name {
    font-weight: 600;
  }
  .queue-progress-bar {
    height: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }
  .progress-fill {
    height: 100%;
    background: var(--color-accent);
    transition: width 0.2s var(--ease-standard);
  }
  .queue-status {
    display: flex;
    justify-content: space-between;
    font-size: 0.7rem;
    opacity: 0.7;
    font-family: var(--font-monospace, monospace);
  }
  .queue-phase {
    font-size: 0.7rem;
    font-family: var(--font-monospace, monospace);
    color: var(--color-accent);
  }
</style>
