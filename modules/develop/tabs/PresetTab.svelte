<script>
  import { onMount } from "svelte";
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";

  /** @typedef {{ id: string, label: string }} EngineInfo */
  let { recipe = $bindable(), /** @type {EngineInfo[]} */ engines = [] } = $props();

  /** @param {string | undefined} id */
  function engineLabel(id) {
    if (!id) return null;
    return engines.find((e) => e.id === id)?.label ?? id;
  }

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
      {#each presets as entry (entry.name)}
        <div class="preset-row">
          <button
            class="apply"
            title="Apply this preset — click: current photo, ⌥-click: whole selection"
            onclick={(e) => applyPreset(entry, e)}
            onmouseenter={() => previewPreset(entry)}
            onmouseleave={clearPresetPreview}
          >
            <span class="preset-name">{entry.name}</span>
            {#if engineLabel(entry.recipe?.engine)}
              <span class="engine-badge">{engineLabel(entry.recipe?.engine)}</span>
            {/if}
          </button>
          <button
            class="ghost icon default-btn"
            class:active={defaultPresetName === entry.name}
            title={defaultPresetName === entry.name
              ? "Default preset on import — click to unset"
              : "Set as default preset on import"}
            onclick={() => toggleDefaultForImport(entry)}
          >
            <Icon name="star" size="11px" />
          </button>
          <button class="ghost icon delete-btn" title="Delete" onclick={() => deletePreset(entry)}>
            <Icon name="x" size="10px" />
          </button>
        </div>
      {/each}
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
  .preset-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .apply {
    all: unset;
    cursor: pointer;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-family: var(--font-text, sans-serif);
    font-size: 11px;
    padding: 8px 11px;
    border-radius: var(--radius);
    border: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
    transition: color var(--duration-fast) var(--ease-soft), border-color var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft), box-shadow var(--duration-fast) var(--ease-soft), transform var(--duration-fast) var(--ease-soft);
  }
  .apply:hover {
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-accent) 45%, var(--color-border));
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
    transform: translateY(-0.5px);
  }
  .preset-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .engine-badge {
    flex-shrink: 0;
    font-family: var(--font-header, sans-serif);
    font-size: 8.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    padding: 2px 6px;
    border-radius: 999px;
  }
  .preset-row button.icon {
    width: 26px;
    height: 26px;
    padding: 0;
    border-radius: var(--radius-sm, 6px);
  }
  .default-btn {
    flex-shrink: 0;
    color: color-mix(in srgb, var(--color-foreground) 35%, transparent);
    transition: color var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft);
  }
  .default-btn:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .default-btn.active {
    color: var(--color-accent);
  }
  .default-btn.active:hover {
    color: var(--color-accent);
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
  }
  .delete-btn {
    color: color-mix(in srgb, var(--color-foreground) 30%, transparent);
    transition: color var(--duration-fast) var(--ease-soft), background var(--duration-fast) var(--ease-soft);
  }
  .delete-btn:hover {
    color: #ef4444;
    background: color-mix(in srgb, #ef4444 12%, transparent);
  }
</style>
