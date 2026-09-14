<script>
  // The THÈME panel — editorial styling for Storytelling.
  // Writes Garden theme tokens into the note's frontmatter via the Rust
  // `story_set_theme` command. Includes quick visual theme chips, a specimen
  // preview card, and fine-tuning controls for colors & typography.
  import { invoke } from "@tauri-apps/api/core";
  import Icon from "$lib/components/Icon.svelte";
  import gardenThemes from "$lib/garden-themes.generated.json";
  import { updateStoryTheme, contrastInk, DEFAULT_DARK_BG, DEFAULT_ACCENT } from "$lib/story-theme.svelte.js";

  let { dir } = $props();

  const THEMES = gardenThemes.themes;

  // Curated prominent themes shown as instant visual chips
  const CURATED_IDS = [
    "reveal",
    "editorial",
    "gallery",
    "book",
    "calm",
    "manifeste",
    "mono",
    "technical"
  ];
  const curatedThemes = THEMES.filter((t) => CURATED_IDS.includes(String(t.id)));

  // All Option<String> — null = unspecified (inherits default).
  /** @type {string | null} */
  let darkBg = $state(null);
  /** @type {string | null} */
  let accent = $state(null);
  /** @type {string | null} */
  let fontHeader = $state(null);
  /** @type {string | null} */
  let fontText = $state(null);
  /** @type {boolean} */
  let customizeOpen = $state(false);

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
    if (!dir) return;
    invoke("story_load_theme", { dir }).then((t) => {
      darkBg = t.darkBackground;
      accent = t.darkAccent;
      fontHeader = t.fontHeader;
      fontText = t.fontText;
    });
  });

  // Sync with global storyTheme so the center story canvas reflects the theme live
  $effect(() => {
    updateStoryTheme({
      darkBackground: darkBg,
      darkAccent: accent,
      fontHeader,
      fontText,
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

  /** @param {typeof THEMES[number]} t */
  function applyTheme(t) {
    darkBg = t.darkBackground;
    accent = t.darkAccent;
    fontHeader = t.fontHeader;
    fontText = t.fontText;
    scheduleSave();
  }

  /** @param {Event & { currentTarget: HTMLSelectElement }} e */
  function onMoreThemePick(e) {
    const id = e.currentTarget.value;
    e.currentTarget.value = "";
    if (!id) return;
    const t = THEMES.find((theme) => String(theme.id) === id);
    if (t) applyTheme(t);
  }

  let previewBg = $derived(darkBg ?? DEFAULT_DARK_BG);
  let previewAccent = $derived(accent ?? DEFAULT_ACCENT);
  let previewFg = $derived(contrastInk(previewBg));



  let currentThemeName = $derived.by(() => {
    const match = THEMES.find(
      (t) =>
        t.darkBackground?.toLowerCase() === darkBg?.toLowerCase() &&
        t.darkAccent?.toLowerCase() === accent?.toLowerCase()
    );
    return match?.label ?? (darkBg ? "Personnalisé" : "Par défaut (Garden)");
  });
</script>

<div class="theme-panel">
  <!-- Specimen card: live atmosphere preview -->
  <div class="specimen" style="background: {previewBg};">
    <div class="specimen-header">
      <span class="specimen-tag" style="color: {previewFg}; opacity: 0.65;">ATMOSPHÈRE</span>
      <span class="specimen-dot" style="background: {previewAccent};"></span>
    </div>
    <div class="specimen-body">
      <span class="specimen-aa" style="color: {previewFg}; font-family: {fontHeader || 'var(--font-header, sans-serif)'};">
        Aa
      </span>
      <div class="specimen-meta">
        <span class="specimen-name" style="color: {previewFg};">{currentThemeName}</span>
        <span class="specimen-fonts" style="color: {previewFg}; opacity: 0.7;">
          {fontHeader || "Système"} · {fontText || "Système"}
        </span>
      </div>
    </div>
  </div>

  <!-- Quick curated themes grid -->
  <div class="section-block">
    <div class="section-title-row">
      <span class="section-label">THÈMES ÉDITORIAUX</span>
      <div class="more-dropdown-wrap">
        <select onchange={onMoreThemePick} aria-label="Tous les thèmes">
          <option value="">Tous les thèmes…</option>
          {#each THEMES as t}
            <option value={t.id}>{t.label}</option>
          {/each}
        </select>
        <Icon name="caret-down" size="9px" class="select-caret" />
      </div>
    </div>

    <div class="chips-grid">
      {#each curatedThemes as t}
        {@const isActive =
          darkBg?.toLowerCase() === t.darkBackground?.toLowerCase() &&
          accent?.toLowerCase() === t.darkAccent?.toLowerCase()}
        <button
          type="button"
          class="theme-chip"
          class:active={isActive}
          onclick={() => applyTheme(t)}
          title={`Appliquer le thème ${t.label}`}
        >
          <span class="chip-swatch" style="background: {t.darkBackground};">
            <span class="chip-accent" style="background: {t.darkAccent};"></span>
          </span>
          <span class="chip-label">{t.label}</span>
        </button>
      {/each}
    </div>
  </div>

  <!-- Fine-tuning custom controls toggle -->
  <div class="customize-block">
    <button
      type="button"
      class="customize-toggle"
      onclick={() => (customizeOpen = !customizeOpen)}
      aria-expanded={customizeOpen}
    >
      <Icon name="sliders" size="11px" />
      <span>Personnaliser les couleurs & polices</span>
      <span class="toggle-arrow" class:open={customizeOpen}>
        <Icon name="caret-right" size="9px" />
      </span>
    </button>

    {#if customizeOpen}
      <div class="customize-fields">
        <!-- Fond color -->
        <div class="field-row">
          <span class="field-label">FOND</span>
          <div class="color-picker-badge">
            <label class="swatch-button" style="background: {previewBg};">
              <input
                type="color"
                value={previewBg}
                oninput={(e) => {
                  darkBg = e.currentTarget.value;
                  scheduleSave();
                }}
              />
            </label>
            <span class="hex-text">{previewBg.toUpperCase()}</span>
            {#if darkBg}
              <button
                type="button"
                class="field-reset"
                title="Rétablir le fond par défaut"
                onclick={() => {
                  darkBg = null;
                  scheduleSave();
                }}
              >
                <Icon name="x" size="10px" />
              </button>
            {/if}
          </div>
        </div>

        <!-- Accent color -->
        <div class="field-row">
          <span class="field-label">ACCENT</span>
          <div class="color-picker-badge">
            <label class="swatch-button" style="background: {previewAccent};">
              <input
                type="color"
                value={previewAccent}
                oninput={(e) => {
                  accent = e.currentTarget.value;
                  scheduleSave();
                }}
              />
            </label>
            <span class="hex-text">{previewAccent.toUpperCase()}</span>
            {#if accent}
              <button
                type="button"
                class="field-reset"
                title="Rétablir l'accent par défaut"
                onclick={() => {
                  accent = null;
                  scheduleSave();
                }}
              >
                <Icon name="x" size="10px" />
              </button>
            {/if}
          </div>
        </div>

        <!-- Font Titres -->
        <div class="field-row">
          <span class="field-label">TITRES</span>
          <div class="custom-select-wrap">
            <select
              value={fontHeader ?? ""}
              onchange={(e) => {
                fontHeader = e.currentTarget.value || null;
                scheduleSave();
              }}
            >
              {#each FONTS as f}
                <option value={f.value ?? ""}>{f.label}</option>
              {/each}
            </select>
            <Icon name="caret-down" size="9px" class="select-caret" />
          </div>
        </div>

        <!-- Font Texte -->
        <div class="field-row">
          <span class="field-label">TEXTE</span>
          <div class="custom-select-wrap">
            <select
              value={fontText ?? ""}
              onchange={(e) => {
                fontText = e.currentTarget.value || null;
                scheduleSave();
              }}
            >
              {#each FONTS as f}
                <option value={f.value ?? ""}>{f.label}</option>
              {/each}
            </select>
            <Icon name="caret-down" size="9px" class="select-caret" />
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .theme-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* Specimen card */
  .specimen {
    position: relative;
    border-radius: var(--radius-md, 10px);
    padding: 12px 14px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: hidden;
    transition: background 0.25s var(--ease-standard);
  }
  .specimen-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .specimen-tag {
    font-family: var(--font-header, sans-serif);
    font-size: 8.5px;
    letter-spacing: 0.14em;
    font-weight: 600;
  }
  .specimen-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    box-shadow: 0 0 8px rgba(0, 0, 0, 0.5);
    transition: background 0.2s ease;
  }
  .specimen-body {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .specimen-aa {
    font-size: 24px;
    font-weight: 600;
    line-height: 1;
    letter-spacing: -0.02em;
    user-select: none;
  }
  .specimen-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .specimen-name {
    font-size: 11.5px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .specimen-fonts {
    font-size: 9.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Section header */
  .section-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .section-title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .section-label {
    font-size: 9px;
    letter-spacing: 0.12em;
    opacity: 0.55;
    font-family: var(--font-header, sans-serif);
    font-weight: 600;
  }

  /* Dropdown wrapper */
  .more-dropdown-wrap,
  .custom-select-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .more-dropdown-wrap select,
  .custom-select-wrap select {
    appearance: none;
    -webkit-appearance: none;
    background: var(--color-surface-high, #222);
    color: var(--color-foreground);
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.12));
    border-radius: var(--radius-sm, 6px);
    padding: 3px 20px 3px 8px;
    font-size: 10px;
    cursor: pointer;
    outline: none;
    font-family: inherit;
    transition: border-color 0.15s ease;
  }
  .custom-select-wrap select {
    width: 120px;
  }
  .more-dropdown-wrap select:hover,
  .custom-select-wrap select:hover {
    border-color: rgba(255, 255, 255, 0.25);
  }
  :global(.select-caret) {
    position: absolute;
    right: 7px;
    pointer-events: none;
    opacity: 0.6;
  }

  /* Chips grid */
  .chips-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 6px;
  }
  .theme-chip {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 8px;
    background: var(--color-surface-low, rgba(255, 255, 255, 0.04));
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.08));
    border-radius: var(--radius-sm, 6px);
    cursor: pointer;
    transition: all 0.15s var(--ease-standard);
  }
  .theme-chip:hover {
    background: var(--color-surface-high, rgba(255, 255, 255, 0.08));
    border-color: rgba(255, 255, 255, 0.2);
  }
  .theme-chip.active {
    background: var(--color-surface-high, rgba(255, 255, 255, 0.12));
    border-color: var(--color-accent, #d6202c);
    box-shadow: 0 0 0 1px var(--color-accent, #d6202c);
  }
  .chip-swatch {
    position: relative;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.2);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .chip-accent {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .chip-label {
    font-size: 11px;
    color: var(--color-foreground);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Customize section */
  .customize-block {
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 1px solid var(--color-border, rgba(255, 255, 255, 0.08));
    padding-top: 8px;
  }
  .customize-toggle {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    opacity: 0.7;
    transition: opacity 0.15s ease;
    padding: 2px 0;
  }
  .customize-toggle:hover {
    opacity: 1;
  }
  .toggle-arrow {
    margin-left: auto;
    transition: transform 0.18s ease;
  }
  .toggle-arrow.open {
    transform: rotate(90deg);
  }
  .customize-fields {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px 0;
  }
  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .field-label {
    font-size: 9.5px;
    letter-spacing: 0.08em;
    opacity: 0.6;
    font-weight: 500;
  }

  /* Color picker badge */
  .color-picker-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--color-surface-high, #222);
    border: 1px solid var(--color-border, rgba(255, 255, 255, 0.1));
    border-radius: var(--radius-sm, 6px);
    padding: 2px 6px 2px 3px;
  }
  .swatch-button {
    position: relative;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.25);
    cursor: pointer;
    overflow: hidden;
    display: inline-block;
  }
  .swatch-button input[type="color"] {
    position: absolute;
    top: -20px;
    left: -20px;
    width: 60px;
    height: 60px;
    opacity: 0;
    cursor: pointer;
  }
  .hex-text {
    font-family: var(--font-monospace, monospace);
    font-size: 9.5px;
    opacity: 0.85;
  }
  .field-reset {
    all: unset;
    cursor: pointer;
    opacity: 0.5;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1px;
  }
  .field-reset:hover {
    opacity: 1;
    color: var(--color-accent);
  }
</style>
