<script>
  // The develop panel's DETACHED host — a separate OS window synchronized
  // with the main window over IPC via createDevPanelMirror.
  import { createDevPanelMirror, DevelopPanel } from "@modules/develop";

  const mirror = createDevPanelMirror();
</script>

<DevelopPanel {...mirror.panelProps} />

<style>
  :global(body) {
    background-color: var(--color-surface-raised);
    color: var(--color-foreground);
    margin: 0;
    padding: 0;
    overflow: hidden;
    height: 100vh;
  }
</style>
