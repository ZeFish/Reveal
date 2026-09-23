<script>
  // The one ambient signal for "something is happening" — subtle,
  // bottom-center, Lightroom-style. Never pops open on its own (Francis: no modal jumping
  // in front of you every time an operation starts); clicking it is the
  // only way to see the full activity panel (old RenderQueueModal, now
  // generic). When several activities run at once, the bar shows their
  // average — the detail panel is where you see each one individually.
  let { activityQueue = [], onOpen = () => {} } = $props();

  const running = $derived(activityQueue.filter((j) => j.status === "running"));
  const aggregatePct = $derived.by(() => {
    if (!running.length) return 0;
    const sum = running.reduce((s, j) => s + (j.total ? Math.min(1, j.done / j.total) : 0), 0);
    return (sum / running.length) * 100;
  });
  const label = $derived(
    running.length > 1 ? `${running.length} activities` : (running[0]?.label ?? "")
  );
</script>

{#if running.length}
  <button class="task-indicator" onclick={() => onOpen()} title={label}>
    <span class="ti-spin" aria-hidden="true"></span>
    <span class="ti-label">{label}</span>
    <span class="ti-track"><span class="ti-fill" style="width: {aggregatePct}%"></span></span>
  </button>
{/if}

<style>
  /* Placement belongs to NotificationStack now, not to each notice. */
  .task-indicator {
    all: unset;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px 5px 8px;
    max-width: 220px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-surface-high, #18181b) 92%, transparent);
    box-shadow: var(--shadow-raised), var(--shadow-lift);
    cursor: pointer;
    font-family: var(--font-monospace, monospace);
    font-size: 10px;
    color: var(--color-foreground, #f4f4f5);
    -webkit-app-region: no-drag;
  }
  .ti-spin {
    width: 9px;
    height: 9px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1.5px solid color-mix(in srgb, var(--color-accent, #d6202c) 35%, transparent);
    border-top-color: var(--color-accent, #d6202c);
    animation: ti-rotate 0.8s linear infinite;
  }
  @keyframes ti-rotate {
    to { transform: rotate(360deg); }
  }
  .ti-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.85;
  }
  .ti-track {
    width: 40px;
    flex-shrink: 0;
    height: 3px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-foreground, #fff) 15%, transparent);
    overflow: hidden;
  }
  .ti-fill {
    display: block;
    height: 100%;
    background: var(--color-accent, #d6202c);
    transition: width 150ms ease;
  }
</style>
