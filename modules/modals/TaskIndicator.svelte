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
  <button class="task-indicator hud" onclick={() => onOpen()} title={label}>
    <span class="loader" aria-hidden="true"></span>
    <span class="ti-label">{label}</span>
    <progress class="ti-track" value={aggregatePct} max="100"></progress>
  </button>
{/if}

<style>
  /* Placement belongs to NotificationStack now, not to each notice. The pill
     itself (background, blur, shadow, radius) is the shared `.hud`, the same one
     Toast wears — without it the label floated bare over the photos. */
  .task-indicator {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    padding: var(--space-d3) calc(var(--space-d4) * 3) var(--space-d3) var(--space-d2);
    max-width: 220px;
    cursor: pointer;
    -webkit-app-region: no-drag;
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
  }
</style>
