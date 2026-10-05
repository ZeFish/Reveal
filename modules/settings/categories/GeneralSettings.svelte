<script>
  import Dropdown from "@stnd/ui/Dropdown.svelte";
  import DropdownItem from "@stnd/ui/DropdownItem.svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";
  import { Icon, isTauri, TEXT_SIZES, DEFAULT_TEXT_SIZE } from "@modules/core";
  import { updater, checkForUpdate, installUpdate } from "@modules/updates";

  /** @typedef {import('../settingsState.svelte.js').Preferences} Preferences */

  /**
   * @typedef {Object} Props
   * @property {Preferences} preferences
   * @property {{id: string, label: string}[]} [themes]
   * @property {(id: string) => Promise<void> | void} [onSelectTheme]
   * @property {(px: number) => void} [onSelectTextSize]
   */

  /** @type {Props} */
  let {
    preferences = $bindable(),
    themes = [],
    onSelectTheme = () => {},
    onSelectTextSize = () => {},
  } = $props();

  let appVersion = $state("");
  $effect(() => {
    if (isTauri) getVersion().then((v) => (appVersion = v)).catch(() => {});
  });

  /** @param {string} url */
  function openExternal(url) {
    invoke("open_path", { path: url }).catch(() => {});
  }
</script>

<div class="section-group">
  <div class="section-heading">
    <Icon name="gear" size="12px" />
    <span>REVEAL</span>
  </div>
  <div class="card flush list divided">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">VERSION</span>
        {#if isTauri && updater.status && updater.status !== "idle"}
          <span class="row-desc">
            {#if updater.status === "checking"}Checking for updates…
            {:else if updater.status === "uptodate"}Reveal is up to date.
            {:else if updater.status === "available"}Reveal {updater.version} is available.
            {:else if updater.status === "downloading"}Downloading {updater.version}…
            {:else if updater.status === "ready"}Restarting…
            {:else if updater.status === "error"}Couldn't check for updates — {updater.error}
            {/if}
          </span>
        {/if}
      </div>
      <div class="row-control version-control">
        {#if isTauri}
          {#if updater.status === "available"}
            <button type="button" class="outline small action-pill-btn" onclick={installUpdate}>
              <span>Install &amp; restart</span>
            </button>
          {:else}
            <button
              type="button"
              class="outline small action-pill-btn"
              disabled={updater.status === "checking" || updater.status === "downloading" || updater.status === "ready"}
              onclick={() => checkForUpdate()}
            >
              <span>{updater.status === "checking" ? "Checking…" : "Check for update"}</span>
            </button>
          {/if}
        {/if}
        <span class="mono">{appVersion || "—"}</span>
      </div>
    </div>
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-desc">Free and open source — a personal darkroom, not a product. Built on the same appetite for crediting the work it stands on that it asks of anyone using it.</span>
      </div>
    </div>
  </div>
</div>

<div class="section-group">
  <div class="section-heading">
    <Icon name="palette" size="12px" />
    <span>APPEARANCE</span>
  </div>
  <div class="card flush list divided date-card">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">THEME</span>
        <span class="row-desc">Applies right away, in every open window — no need to Save.</span>
      </div>
      <div class="row-control">
        <Dropdown label="Theme" triggerClass="outline small action-pill-btn" align="end">
          {#snippet trigger()}
            <span>{themes.find((t) => t.id === preferences.app_theme)?.label ?? "Reveal"}</span>
            <Icon name="caret-down" size="10px" />
          {/snippet}
          {#each themes as theme}
            <DropdownItem onclick={() => onSelectTheme(theme.id)}>{theme.label}</DropdownItem>
          {/each}
        </Dropdown>
      </div>
    </div>
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">TEXT SIZE</span>
        <span class="row-desc">Scales the whole interface together — text, spacing and controls.</span>
      </div>
      <div class="row-control">
        <Dropdown label="Text size" triggerClass="outline small action-pill-btn" align="end">
          {#snippet trigger()}
            <span>{TEXT_SIZES.find((t) => t.px === (preferences.ui_text_size ?? DEFAULT_TEXT_SIZE))?.label ?? "Default"}</span>
            <Icon name="caret-down" size="10px" />
          {/snippet}
          {#each TEXT_SIZES as size}
            <DropdownItem onclick={() => onSelectTextSize(size.px)}>{size.label} · {size.px}px</DropdownItem>
          {/each}
        </Dropdown>
      </div>
    </div>
  </div>
</div>

<div class="section-group">
  <div class="section-heading">
    <Icon name="heart" size="12px" />
    <span>OPEN SOURCE &amp; CREDITS</span>
  </div>
  <div class="card flush list divided">
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">SPEKTRAFILM-RS</span>
        <span class="row-desc">The film-emulation engine behind the Spektra develop mode — turbasvin's Rust port of spektrafilm, pinned per release.</span>
      </div>
      <div class="row-control">
        <button type="button" class="outline small action-pill-btn" onclick={() => openExternal("https://github.com/turbasvin/spektrafilm-rs")}>
          <Icon name="arrow-square-out" size="10px" />
          <span>GitHub</span>
        </button>
      </div>
    </div>
    <div class="setting-row">
      <div class="row-meta">
        <span class="row-label">RAPIDRAW</span>
        <span class="row-desc">Timon Käch's GPU-accelerated RAW editor (AGPL-3.0) — a source of real inspiration for where Reveal's own develop engine can go.</span>
      </div>
      <div class="row-control">
        <button type="button" class="outline small action-pill-btn" onclick={() => openExternal("https://github.com/CyberTimon/RapidRAW")}>
          <Icon name="arrow-square-out" size="10px" />
          <span>GitHub</span>
        </button>
      </div>
    </div>
  </div>
</div>
