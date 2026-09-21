<script>
  // The one place brief text messages appear — top-center, a few seconds,
  // then gone. Every call site still just sets `appMessage = "…"` and clears
  // it with its own setTimeout exactly as before; this only changes WHERE
  // that text is shown (a floating pill, not a static span buried in the
  // toolbar) — no call site needed to change for this to land everywhere at
  // once.
  let { message = "" } = $props();
</script>

{#if message}
  <div class="toast" role="status">{message}</div>
{/if}

<style>
  .toast {
    position: fixed;
    top: 14px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 90;
    max-width: min(32rem, calc(100vw - 2rem));
    padding: 0.5rem 1rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--color-surface-high, #18181b) 94%, transparent);
    border: 1px solid var(--color-border, color-mix(in srgb, #fff 10%, transparent));
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.3);
    color: var(--color-foreground, #f4f4f5);
    font-family: var(--font-monospace, monospace);
    font-size: 11px;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    -webkit-app-region: no-drag;
  }
</style>
