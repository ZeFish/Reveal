<script>
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri } from "$lib/api.js";
  import { applyTheme, syncThemeTransition } from "$lib/app-theme.js";
  // The Standard visual identity — framework-agnostic pieces of the monorepo.
  import "@stnd/styles/standard.scss";
  // Fonts and theme are not reachable through their packages' exports maps
  // (fonts' "./*/*" double-star pattern isn't valid Node resolution), so both
  // are imported by monorepo path.
  import "../../../../packages/fonts/inter/inter.css";
  import "../../../../packages/fonts/din-condensed/din-condensed.css";
  import "../../../../packages/fonts/ibm-plex/ibm-plex.css";
  import "../../../../packages/fonts/newsreader/newsreader.css";
  import "../../../../packages/themes/reveal/reveal.scss";
  // App adapter — re-grounds the note framework's tokens for an app window
  // (fixed UI scale, no mobile bump, no reading measure). Must come last.
  import "../app.scss";

  let { children } = $props();

  onMount(() => {
    document.documentElement.classList.add("js-image-zoom-enabled");

    // Every Reveal window (main, dev-panel, settings-panel, import-panel)
    // loads this layout — the one place to boot the saved app theme and
    // stay in sync when another window changes it live.
    if (!isTauri) return;
    invoke("load_preferences")
      .then((prefs) => applyTheme(/** @type {any} */ (prefs)?.app_theme))
      .catch(() => {});
    const unlisten = listen("app-theme-changed", (e) => {
      applyTheme(/** @type {any} */ (e.payload)?.app_theme);
    });

    // Themes carry a --color-light-*/--color-dark-* pair each, resolved by
    // the media query below — so a macOS system light/dark toggle recolors
    // just as much of the UI as picking a whole new theme does, and needs
    // the same transition sync.
    const scheme = window.matchMedia("(prefers-color-scheme: dark)");
    const onSchemeChange = () => syncThemeTransition();
    scheme.addEventListener("change", onSchemeChange);

    return () => {
      unlisten.then((fn) => fn());
      scheme.removeEventListener("change", onSchemeChange);
    };
  });
</script>

{@render children()}

<style>
  :global(:root) {
    --window-controls-offset-sidebar: 78px;
    /* The photo canvas: the ground plus the framework's sunk wash. */
    --canvas: linear-gradient(var(--color-surface-lower), var(--color-surface-lower))
      var(--color-background);
    --window-controls-offset-content: 86px;
  }
  /* The app's ground is the framework's own --color-background — what the
     sidebar sits on. The photo canvas is recessed below it (see .dev-grid in
     +page.svelte). The app-vs-website resets (measure, margins, user-select)
     live in app.scss — this block is only reveal's visual ground. */
  :global(body:not(.is-transparent-window)) {
    background-color: var(--color-background);
    color: var(--color-foreground);
    font-family: var(--font-text, system-ui, sans-serif);
    -webkit-font-smoothing: antialiased;
  }
  /* Quiet capsule buttons, app-wide (the Swift control language). */
  :global(button) {
    font-family: var(--font-monospace, monospace);
    font-size: 12px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--color-foreground);
    background: var(--color-surface-high);
    border: var(--border);
    border-radius: 999px;
    padding: 0.32rem 0.8rem;
    cursor: pointer;
  }
  :global(button:hover:not(:disabled)) {
    border-color: var(--color-accent);
  }
  :global(button:disabled) {
    opacity: 0.45;
    cursor: default;
  }
  :global(select) {
    font-family: var(--font-monospace, monospace);
    font-size: 12px;
    color: var(--color-foreground);
    background: var(--color-surface-high);
    border: var(--border);
    border-radius: var(--radius);
    padding: 0.2rem 0.35rem;
  }
</style>
