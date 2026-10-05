<script>
  // Settings' DETACHED host — always detached, no docked mode.
  // Synchronizes mirror state with main window via createSettingsPanelMirror.
  import { createSettingsPanelMirror, SettingsPanel } from "@modules/settings";

  const mirror = createSettingsPanelMirror();
</script>

<SettingsPanel {...mirror.panelProps} />
