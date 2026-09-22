<script>
  // One corner of the window owns every floating notice. Before this, a
  // message could appear top-center (Toast), bottom-center (TaskIndicator)
  // or inside the develop canvas (the render badge), and you had to know
  // which kind of event you'd triggered to know where to look.
  //
  // Order is deliberate: transient messages sit ABOVE the persistent
  // activity indicator, so a toast arriving mid-import pushes up and away
  // rather than displacing the thing you're watching.
  let { children } = $props();
</script>

<div class="notification-stack">
  {@render children?.()}
</div>

<style>
  .notification-stack {
    position: fixed;
    bottom: 12px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 90;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    /* The stack spans the window so its children can centre themselves, but
       it must never swallow clicks meant for the photo behind it — each
       child opts back in. */
    pointer-events: none;
  }
  .notification-stack > :global(*) {
    pointer-events: auto;
  }
</style>
