<script>
  // The import HUD's DETACHED host — a transparent floating window
  // synchronized with the main app and card events over Tauri IPC.
  import { createImportPanelMirror, ImportPanel } from "@modules/import";

  const mirror = createImportPanelMirror();
</script>

<ImportPanel {...mirror.panelProps} />
