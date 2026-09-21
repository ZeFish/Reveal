<script>
  import { onMount, onDestroy } from "svelte";
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";

  /** @typedef {{ id: string, label: string }} EngineInfo */
  let {
    recipe = $bindable(),
    /** @type {EngineInfo[]} */
    engines = [],
    photoPath = null,
  } = $props();

  /** @param {string | undefined} id */
  function engineLabel(id) {
    if (!id) return null;
    return engines.find((e) => e.id === id)?.label ?? id;
  }

  // Gallery thumbnails — the current photo re-developed through each
  // preset's own recipe (the same `develop_preview` command the main
  // canvas uses), so what you see is exactly what applying it would give
  // you, not a generic swatch. Small max_px (Rust caches the render per
  // exact recipe hash, so revisiting this tab or reopening the same photo
  // is a cache hit, not a re-render). Keyed by photo+name rather than
  // wiped on every `presets` refresh, so saving/deleting one preset
  // doesn't throw away everyone else's already-rendered thumbnail.
  const THUMB_PX = 160;
  /** @type {Map<string, string>} `${path}::${name}` -> blob url */
  let thumbCache = $state(new Map());
  /** @param {string} path @param {string} name */
  const thumbKey = (path, name) => `${path}::${name}`;

  /** @param {string | null} path @param {Array<{name: string, recipe: any}>} list */
  async function ensureThumbnails(path, list) {
    if (!path || !isTauri) return;
    for (const entry of list) {
      const key = thumbKey(path, entry.name);
      if (thumbCache.has(key)) continue;
      try {
        const bytes = await invoke("develop_preview", { path, recipe: entry.recipe, maxPx: THUMB_PX });
        // A newer photo or a deleted preset may have superseded this by
        // the time the render comes back — still cache it (harmless, and
        // cheap if revisited), just don't bother if it's now stale.
        if (path !== photoPath) continue;
        thumbCache.set(key, URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" })));
      } catch (_) {
        // leave uncached — the card falls back to its skeleton placeholder
      }
    }
  }

  $effect(() => {
    ensureThumbnails(photoPath, presets);
  });

  onDestroy(() => {
    for (const url of thumbCache.values()) URL.revokeObjectURL(url);
  });

  /** @type {Array<{ name: string, recipe: any }>} */
  let presets = $state([]);
  let newName = $state("");
  let busy = $state(false);
  // The preset (by name) auto-applied to every photo as it lands from a card
  // import — Rust's ShellPrefs.default_import_preset, mirrored here so the
  // star below reflects it live regardless of which window last changed it.
  /** @type {string | null} */
  let defaultPresetName = $state(null);

  async function refresh() {
    if (!isTauri) return;
    try {
      presets = await invoke("list_presets");
    } catch (_) {
      presets = [];
    }
  }

  onMount(() => {
    if (!isTauri) {
      presets = [
        { name: "Golden Hour", recipe: {} },
        { name: "Bleach Bypass", recipe: {} },
      ];
      return;
    }
    const unlistenChanged = listen("presets-changed", refresh);
    const unlistenPrefs = listen("shell-prefs-changed", (e) => {
      defaultPresetName = e.payload?.default_import_preset ?? null;
    });
    refresh();
    invoke("load_shell_prefs").then((prefs) => {
      defaultPresetName = prefs?.default_import_preset ?? null;
    });

    return () => {
      unlistenChanged.then((fn) => fn());
      unlistenPrefs.then((fn) => fn());
    };
  });

  /**
   * Toggle a preset as THE default applied automatically on import — a
   * second click on the already-default one clears it back to "none".
   * @param {{ name: string }} entry
   */
  async function toggleDefaultForImport(entry) {
    const next = defaultPresetName === entry.name ? null : entry.name;
    defaultPresetName = next; // optimistic; shell-prefs-changed will confirm
    try {
      await invoke("set_default_import_preset", { name: next });
    } catch (_) {
      // reverted by the next shell-prefs-changed broadcast if this failed
    }
  }

  async function saveCurrent() {
    const name = newName.trim();
    if (!name || !recipe || busy) return;
    busy = true;
    try {
      await invoke("save_preset", { name, recipe: { ...recipe } });
      newName = "";
      await refresh();
      emit("presets-changed", {});
    } catch (_) {
    } finally {
      busy = false;
    }
  }

  /**
   * @param {{ name: string, recipe: any }} entry
   */
  function previewPreset(entry) {
    emit("preset-preview", {
      recipe: entry.recipe,
    });
  }

  function clearPresetPreview() {
    emit("preset-preview", {
      recipe: null,
    });
  }

  /**
   * @param {{ name: string, recipe: any }} entry
   * @param {MouseEvent & { altKey?: boolean }} [ev]
   */
  function applyPreset(entry, ev) {
    emit("preset-apply", {
      recipe: entry.recipe,
      scope: ev?.altKey ? "selected" : "auto",
    });
  }

  /**
   * @param {{ name: string, recipe: any }} entry
   */
  async function deletePreset(entry) {
    if (busy) return;
    busy = true;
    try {
      await invoke("delete_preset", { name: entry.name });
      if (defaultPresetName === entry.name) {
        // Rust already tolerates a stale/missing name here (silently no-ops
        // at import time), but leaving prefs.json pointing at a preset that
        // no longer exists is just confusing — clear it explicitly.
        await invoke("set_default_import_preset", { name: null });
      }
      await refresh();
      emit("presets-changed", {});
    } catch (_) {
    } finally {
      busy = false;
    }
  }
</script>

<div class="pane-scroll">
  <section>
    <span class="din section-label">Save current</span>
    <div class="save-row">
      <input
        class="panel-input name-input"
        placeholder="Preset name…"
        bind:value={newName}
        onkeydown={(e) => e.key === "Enter" && saveCurrent()}
      />
      <button class="secondary save-btn" onclick={saveCurrent} disabled={!newName.trim() || !recipe || busy}>
        <Icon name="plus" size="10px" />
        <span>Save</span>
      </button>
    </div>
    <p class="hint">Click applies to the current photo · ⌥-click applies to the whole selection</p>
  </section>

  <div class="hairline-inner"></div>

  <section class="list">
    <div class="list-header">
      <span class="din section-label">Presets</span>
      {#if presets.length > 0}
        <span class="count-badge">{presets.length}</span>
      {/if}
    </div>
    {#if presets.length === 0}
      <div class="empty-state">
        <Icon name="stack-simple" size="20px" />
        <p class="empty-text">No presets yet — save the current settings above to start a library.</p>
      </div>
    {:else}
      <div class="preset-grid">
        {#each presets as entry (entry.name)}
          {@const thumb = photoPath ? thumbCache.get(thumbKey(photoPath, entry.name)) : null}
          <div
            class="preset-card"
            role="button"
            tabindex="0"
            title="Apply this preset — click: current photo, ⌥-click: whole selection"
            onclick={(e) => applyPreset(entry, e)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                applyPreset(entry);
              }
            }}
            onmouseenter={() => previewPreset(entry)}
            onmouseleave={clearPresetPreview}
          >
            <div class="thumb-wrap">
              {#if thumb}
                <img class="thumb" src={thumb} alt={entry.name} />
              {:else}
                <div class="thumb-skeleton"></div>
              {/if}
              <div class="card-actions">
                <button
                  class="ghost icon default-btn"
                  class:active={defaultPresetName === entry.name}
                  title={defaultPresetName === entry.name
                    ? "Default preset on import — click to unset"
                    : "Set as default preset on import"}
                  onclick={(e) => {
                    e.stopPropagation();
                    toggleDefaultForImport(entry);
                  }}
                >
                  <Icon name="star" size="10px" />
                </button>
                <button
                  class="ghost icon delete-btn"
                  title="Delete"
                  onclick={(e) => {
                    e.stopPropagation();
                    deletePreset(entry);
                  }}
                >
                  <Icon name="x" size="9px" />
                </button>
              </div>
            </div>
            <div class="card-meta">
              <span class="preset-name">{entry.name}</span>
              {#if engineLabel(entry.recipe?.engine)}
                <span class="engine-badge">{engineLabel(entry.recipe?.engine)}</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .hairline-inner {
    height: 1px;
    background: var(--color-border);
    opacity: 0.6;
  }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }
  .section-label {
    font-size: 9.5px;
  }
  .list-header {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .count-badge {
    font-family: var(--font-monospace, monospace);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    padding: 1px 6px;
    border-radius: 999px;
  }

  .save-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  /* Sizing only — border/background/focus ring on inputs and buttons come
     from Standard's own zero-class rules (_standard-11-forms.scss,
     _standard-13-components.scss) plus the app-wide pill shape in
     +layout.svelte. */
  .panel-input {
    flex: 1;
    min-width: 0;
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    padding: 7px 9px;
  }
  .save-btn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    padding: 7px 13px;
  }

  .hint {
    font-family: var(--font-text, sans-serif);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
    margin: 0;
  }

  .list {
    gap: 6px;
  }
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 28px 16px;
    color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
    border: 1px dashed var(--color-border);
    border-radius: var(--radius);
  }
  .empty-text {
    font-family: var(--font-text, sans-serif);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    line-height: 1.4;
    text-align: center;
    margin: 0;
  }
  /* Gallery grid — two columns of square-ish thumbnail cards, the current
     photo re-developed through each preset (see ensureThumbnails above).
     Star/delete ride as a top-right overlay on the thumbnail itself rather
     than living in their own row, since a card's job now IS the image. */
  .preset-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
  .preset-card {
    all: unset;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 5px;
    border-radius: var(--radius);
    transition: transform var(--duration-fast) var(--ease-soft);
  }
  .preset-card:hover {
    transform: translateY(-1px);
  }
  .thumb-wrap {
    position: relative;
    aspect-ratio: 1;
    border-radius: var(--radius);
    overflow: hidden;
    border: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
    transition: border-color var(--duration-fast) var(--ease-soft), box-shadow var(--duration-fast) var(--ease-soft);
  }
  .preset-card:hover .thumb-wrap {
    border-color: color-mix(in srgb, var(--color-accent) 45%, var(--color-border));
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.28);
  }
  .thumb {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .thumb-skeleton {
    width: 100%;
    height: 100%;
    background: linear-gradient(
      100deg,
      color-mix(in srgb, var(--color-foreground) 5%, transparent) 30%,
      color-mix(in srgb, var(--color-foreground) 10%, transparent) 50%,
      color-mix(in srgb, var(--color-foreground) 5%, transparent) 70%
    );
    background-size: 200% 100%;
    animation: preset-thumb-pulse 1.4s ease-in-out infinite;
  }
  @keyframes preset-thumb-pulse {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }
  .card-actions {
    position: absolute;
    top: 4px;
    right: 4px;
    display: flex;
    gap: 3px;
    opacity: 0;
    transition: opacity var(--duration-fast) var(--ease-soft);
  }
  .preset-card:hover .card-actions,
  .preset-card:focus-visible .card-actions,
  .card-actions:has(.default-btn.active) {
    opacity: 1;
  }
  .card-actions button.icon {
    width: 20px;
    height: 20px;
    padding: 0;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(4px);
    color: rgba(255, 255, 255, 0.85);
  }
  .card-actions button.icon:hover {
    background: rgba(0, 0, 0, 0.75);
    color: #fff;
  }
  .default-btn.active {
    color: var(--color-accent);
  }
  .card-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 0 1px;
  }
  .preset-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-text, sans-serif);
    font-size: 10.5px;
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }
  .engine-badge {
    flex-shrink: 0;
    font-family: var(--font-header, sans-serif);
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
  }
</style>
