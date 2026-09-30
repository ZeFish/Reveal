<script>
  // Brief text messages — a fact that just became true ("published ✓",
  // "Copied link"), not work in progress. Lives at the bottom of the window
  // in the same pill language as TaskIndicator, stacked above it by
  // NotificationStack so the two never sit on top of each other.
  //
  // This is THE message channel. The develop canvas's `status` used to carry
  // messages too, which is how the same event ended up announced in two
  // places at once (Francis, 2026-09-21: "I keep seeing different toast at
  // different place"). `status` now only means "what this photo's render is
  // doing"; anything the user should READ comes through here.
  let { message = "" } = $props();
</script>

{#if message}
  <div class="toast hud" role="status">{message}</div>
{/if}

<style>
  .toast {
    max-width: min(32rem, calc(100vw - 2rem));
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    -webkit-app-region: no-drag;
    animation: toast-in 160ms ease-out;
  }
  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
