<script>
  import { onMount } from "svelte";
  import { listen, emit } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "$lib/api.js";
  import Icon from "$lib/components/Icon.svelte";

  let { recipe = $bindable() } = $props();

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
    <div class="save-row">
      <input
        class="panel-input name-input"
        placeholder="Nom du preset"
        bind:value={newName}
        onkeydown={(e) => e.key === "Enter" && saveCurrent()}
      />
      <button class="secondary save-btn" onclick={saveCurrent} disabled={!newName.trim() || !recipe || busy}>
        Enregistrer
      </button>
    </div>
    <p class="hint">Clic : photo courante · ⌥-clic : toute la sélection · ★ : préset par défaut à l'import</p>
  </section>

  <div class="hairline-inner"></div>

  <section class="list">
    {#if presets.length === 0}
      <p class="empty-text">Aucun preset. Enregistre le réglage courant ci-dessus.</p>
    {:else}
      {#each presets as entry (entry.name)}
        <div class="preset-row">
          <button
            class="apply"
            title="Appliquer ce preset"
            onclick={(e) => applyPreset(entry, e)}
            onmouseenter={() => previewPreset(entry)}
            onmouseleave={clearPresetPreview}
          >
            {entry.name}
          </button>
          <button
            class="ghost icon default-btn"
            class:active={defaultPresetName === entry.name}
            title={defaultPresetName === entry.name
              ? "Preset par défaut à l'import — clic pour retirer"
              : "Définir comme preset par défaut à l'import"}
            onclick={() => toggleDefaultForImport(entry)}
          >
            <Icon name="star" size="11px" />
          </button>
          <button class="ghost icon" title="Supprimer" onclick={() => deletePreset(entry)}>×</button>
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
    gap: 16px;
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
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
    padding: 6px 8px;
  }
  .save-btn {
    flex-shrink: 0;
    font-size: 10px;
    padding: 6px 12px;
  }

  .hint {
    font-family: var(--font-text, sans-serif);
    font-size: 9px;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
    margin: 0;
  }

  .list {
    gap: 4px;
  }
  .empty-text {
    font-family: var(--font-monospace, monospace);
    font-size: 10.8px;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    line-height: 1.4;
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
    text-align: left;
    font-family: var(--font-text, sans-serif);
    font-size: 11px;
    padding: 6px 10px;
    border-radius: var(--radius);
    border: 1px solid var(--color-border);
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--duration-instant) var(--ease-soft), border-color var(--duration-instant) var(--ease-soft), background var(--duration-instant) var(--ease-soft);
  }
  .apply:hover {
    color: var(--color-foreground);
    border-color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
  }
  .preset-row button.icon {
    width: 26px;
    height: 26px;
    padding: 0;
  }
  .default-btn {
    flex-shrink: 0;
    color: color-mix(in srgb, var(--color-foreground) 35%, transparent);
  }
  .default-btn.active {
    color: var(--color-accent);
    border-color: var(--color-accent);
  }
</style>
