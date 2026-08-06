<script>
  // The THÈME panel — a port of Swift `StoryThemeView` (StoryTheme.swift).
  // Writes Garden theme tokens into the note's frontmatter via the Rust
  // `story_set_theme` command (read-modify-write from disk, invariant 3).
  // The user picks FOND (dark bg) + ACCENT + two fonts; the light side and
  // both foregrounds are derived in Rust. Defaults = unspecified (None):
  // a fresh note has no theme keys and inherits the Garden default.
  import { invoke } from "@tauri-apps/api/core";
  import gardenThemes from "$lib/garden-themes.generated.json";

  let { dir } = $props();

  const THEMES = gardenThemes.themes;

  // All Option<String> — null = unspecified (inherits default).
  /** @type {string | null} */
  let darkBg = $state(null);
  /** @type {string | null} */
  let accent = $state(null);
  /** @type {string | null} */
  let fontHeader = $state(null);
  /** @type {string | null} */
  let fontText = $state(null);

  // Display defaults shown in the picker when a token is unspecified.
  const DEFAULT_DARK_BG = "#15110D";
  const DEFAULT_ACCENT = "#D6202C";

  // The curated @stnd/fonts set — port of Swift `storyFonts` (StoryTheme.swift:11-23).
  // "" value = None (system / inherits default).
  const FONTS = [
    { label: "Système", value: null },
    { label: "Söhne", value: "Sohne" },
    { label: "Avenir Next", value: "Avenir Next" },
    { label: "Lexend", value: "Lexend" },
    { label: "Instrument Sans", value: "Instrument Sans" },
    { label: "Baskerville", value: "Baskerville" },
    { label: "Bookerly", value: "Bookerly" },
    { label: "Adobe Jenson Pro", value: "Adobe Jenson Pro" },
    { label: "New Burns", value: "New Burns" },
    { label: "National Park", value: "National Park" },
    { label: "Wonder", value: "Wonder" },
  ];

  /** @type {ReturnType<typeof setTimeout> | null} */
  let saveTimer = null;

  $effect(() => {
    // reload when the folder changes
    if (!dir) return;
    invoke("story_load_theme", { dir }).then((t) => {
      darkBg = t.darkBackground;
      accent = t.darkAccent;
      fontHeader = t.fontHeader;
      fontText = t.fontText;
    });
  });

  async function persist() {
    if (!dir) return;
    await invoke("story_set_theme", {
      dir,
      tokens: {
        darkBackground: darkBg,
        darkForeground: null,
        lightBackground: null,
        lightForeground: null,
        darkAccent: accent,
        lightAccent: null,
        fontHeader,
        fontText,
      },
    });
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(persist, 250);
  }

  // Applying a named Standard theme is a one-shot fill, not a persistent
  // selection — there's no "which theme this is" field in the saved tokens,
  // and the swatches below stay the source of truth for fine-tuning after.
  /** @param {Event & { currentTarget: HTMLSelectElement }} e */
  function onThemePick(e) {
    const id = e.currentTarget.value;
    e.currentTarget.value = "";
    if (!id) return;
    const t = THEMES.find((theme) => String(theme.id) === id);
    if (!t) return;
    darkBg = t.darkBackground;
    accent = t.darkAccent;
    fontHeader = t.fontHeader;
    fontText = t.fontText;
    scheduleSave();
  }

  // The live preview swatch uses the effective bg (picked or default).
  let previewBg = $derived(darkBg ?? DEFAULT_DARK_BG);
  let previewAccent = $derived(accent ?? DEFAULT_ACCENT);
  // Contrast-derived foreground for the preview text — matches Rust derive_foreground.
  let previewFg = $derived(contrastInk(previewBg));

  /** @param {string} hex */
  function contrastInk(hex) {
    const s = hex.replace("#", "");
    if (s.length !== 6) return "#15110D";
    const v = parseInt(s, 16);
    const r = ((v >> 16) & 0xff) / 255;
    const g = ((v >> 8) & 0xff) / 255;
    const b = (v & 0xff) / 255;
    const lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    return lum < 0.5 ? "#F5F1EA" : "#15110D";
  }
</script>

<div class="theme-panel">
  <!-- Live swatch: fond + derived fg "Aa" + accent dot -->
  <div class="swatch" style="background:{previewBg}">
    <span class="dot" style="background:{previewAccent}"></span>
    <span class="aa" style="color:{previewFg}">Aa</span>
  </div>

  <div class="row">
    <span class="label">THÈME</span>
    <select value="" onchange={onThemePick}>
      <option value="">Personnalisé…</option>
      {#each THEMES as t}
        <option value={t.id}>{t.label}</option>
      {/each}
    </select>
  </div>

  <div class="row">
    <span class="label">FOND</span>
    <div class="color-cell">
      <input
        type="color"
        value={previewBg}
        oninput={(e) => { darkBg = e.currentTarget.value; scheduleSave(); }}
      />
      {#if darkBg}
        <button class="reset" title="Réinitialiser" onclick={() => { darkBg = null; scheduleSave(); }}>×</button>
      {/if}
    </div>
  </div>

  <div class="row">
    <span class="label">ACCENT</span>
    <div class="color-cell">
      <input
        type="color"
        value={previewAccent}
        oninput={(e) => { accent = e.currentTarget.value; scheduleSave(); }}
      />
      {#if accent}
        <button class="reset" title="Réinitialiser" onclick={() => { accent = null; scheduleSave(); }}>×</button>
      {/if}
    </div>
  </div>

  <p class="hint">Tu travailles le sombre — le clair est l'inverse, l'accent suit.</p>

  <hr />

  <div class="row">
    <span class="label">TITRES</span>
    <select value={fontHeader ?? ""} onchange={(e) => { fontHeader = e.currentTarget.value || null; scheduleSave(); }}>
      {#each FONTS as f}
        <option value={f.value ?? ""}>{f.label}</option>
      {/each}
    </select>
  </div>

  <div class="row">
    <span class="label">TEXTE</span>
    <select value={fontText ?? ""} onchange={(e) => { fontText = e.currentTarget.value || null; scheduleSave(); }}>
      {#each FONTS as f}
        <option value={f.value ?? ""}>{f.label}</option>
      {/each}
    </select>
  </div>
</div>

<style>
  .theme-panel { display: flex; flex-direction: column; gap: 10px; }
  .swatch {
    height: 44px; border-radius: var(--radius); display: flex; align-items: center;
    gap: 10px; padding: 0 14px; border: 1px solid var(--border, rgba(255,255,255,0.1));
  }
  .swatch .dot { width: 12px; height: 12px; border-radius: 50%; flex-shrink: 0; }
  .swatch .aa { font-size: 18px; font-weight: 600; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .label { font-size: 10px; letter-spacing: 0.08em; opacity: 0.6; }
  .color-cell { display: flex; align-items: center; gap: 4px; }
  .color-cell input[type="color"] {
    width: 28px; height: 20px; border: 1px solid var(--color-border);
    border-radius: var(--radius-sm); background: none; cursor: pointer; padding: 0;
  }
  .reset {
    background: none; border: none; color: inherit; cursor: pointer;
    opacity: 0.5; font-size: 14px; line-height: 1; padding: 0 2px;
  }
  .reset:hover { opacity: 1; }
  .hint { font-size: 10px; opacity: 0.5; margin: 0; }
  hr { border: none; border-top: 1px solid var(--border, rgba(255,255,255,0.1)); margin: 2px 0; }
  select {
    background: var(--color-surface-low); color: inherit; border: 1px solid var(--color-border);
    border-radius: var(--radius-sm); padding: 2px 6px; font-size: 11px; min-width: 110px;
  }
</style>
