<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri, applyTheme, applyTextSize } from "@modules/core";
  // The Standard visual identity — framework-agnostic pieces of the monorepo.
  import "@stnd/styles/standard.scss";
  // Fonts and theme are not reachable through their packages' exports maps
  // (fonts' "./*/*" double-star pattern isn't valid Node resolution), so both
  // are imported by monorepo path.
  import "../../../../packages/fonts/inter/inter.css";
  import "../../../../packages/fonts/din-condensed/din-condensed.css";
  import "../../../../packages/fonts/ibm-plex/ibm-plex.css";
  import "../../../../packages/fonts/newsreader/newsreader.css";
  import "../../../../packages/themes/macos/macos.scss";
  // App adapter — re-grounds the note framework's tokens for an app window
  // (fixed UI scale, no mobile bump, no reading measure). Must come last.
  import "../app.scss";

  let { children } = $props();

  onMount(() => {
    // Every Reveal window (main, dev-panel, settings-panel, import-panel)
    // loads this layout — the one place to boot the saved app theme and
    // stay in sync when another window changes it live.
    if (!isTauri) return;
    invoke("load_preferences")
      .then((prefs) => {
        applyTheme(/** @type {any} */ (prefs)?.app_theme);
        applyTextSize(/** @type {any} */ (prefs)?.ui_text_size);
      })
      .catch(() => {});
    const unlisten = listen("app-theme-changed", (e) => {
      applyTheme(/** @type {any} */ (e.payload)?.app_theme);
    });

    const unlistenSize = listen("app-text-size-changed", (e) => {
      applyTextSize(/** @type {any} */ (e.payload)?.ui_text_size);
    });

    return () => {
      unlisten.then((fn) => fn());
      unlistenSize.then((fn) => fn());
    };
  });
</script>

{@render children()}

<style>
  :global(:root) {
    --window-controls-offset-sidebar: 66px;
    /* The photo canvas: unified with the global ground. */
    --canvas: var(--color-background);
    --window-controls-offset-content: 86px;
  }
</style>
