<script>
  import Icon from "$lib/components/Icon.svelte";

  let {
    engine,
    recipe = $bindable(),
    films = [],
    papers = [],
    luts = [],
    edited = () => {},
    resetControl = () => {}, // double-click a slider → its engine default
    addLutLayer = () => {},
    removeLutLayer = () => {},
    updateLutOpacity = () => {},
    setLutFile = () => {},
  } = $props();

  // Collapsible control groups, remembered per-label across sessions.
  const COLLAPSE_KEY = "reveal.engineRunner.collapsedGroups";
  function readCollapsedGroups() {
    try {
      return new Set(JSON.parse(localStorage.getItem(COLLAPSE_KEY) || "[]"));
    } catch {
      return new Set();
    }
  }
  let collapsedGroups = $state(readCollapsedGroups());

  /** @param {string} label */
  function toggleGroup(label) {
    const next = new Set(collapsedGroups);
    if (next.has(label)) next.delete(label);
    else next.add(label);
    collapsedGroups = next;
    try {
      localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...next]));
    } catch {}
  }

  /** @param {string} stage */
  function lutListFor(stage) {
    if (stage === "pre") return recipe.rapid_pre_luts?.length ? recipe.rapid_pre_luts : (recipe.pre_luts || []);
    return recipe.rapid_post_luts?.length ? recipe.rapid_post_luts : (recipe.post_luts || []);
  }

  /** @type {(v: number | undefined, min: number, max: number) => string} */
  const pct = (v, min, max) => `${((Number(v ?? 0) - min) / (max - min)) * 100}%`;

  // Francis: "le curseur tirage, on pourrait l'inverser?" — print_exposure_ev
  // runs -3..3 in the engine (more EV = more exposure = a darker print, the
  // real-darkroom convention), which reads backwards next to every other
  // slider here. The range is symmetric around 0, so flipping the sign for
  // display/input is a pure UI mirror — right still ends up "brighter" to
  // match the rest of the panel, and the stored recipe value (what the
  // pipeline actually consumes) never changes meaning.
  const INVERTED_CONTROLS = new Set(["print_exposure_ev"]);
  /** @param {string} id @param {number | undefined} v */
  const toDisplay = (id, v) => (INVERTED_CONTROLS.has(id) ? -(v ?? 0) : (v ?? 0));
  /** @param {string} id @param {number} v */
  const fromDisplay = (id, v) => (INVERTED_CONTROLS.has(id) ? -v : v);

  let isPositive = $derived(
    films.find((f) => f.name === recipe.film)?.film_type === "positive"
  );
  // spektrafilm-rs's resolve_for_render only varies by development_time for
  // a "bw" channel_model profile (it collapses a family of push/pull density
  // curves to the selected one) — every color profile ignores the value
  // entirely. Confirmed by reading the dependency's source after Francis
  // reported "Durée" doing nothing on Kodak Gold 200 / Kodachrome 64 (both
  // color) — not a wiring bug, just a control that's a no-op outside B&W.
  let isBw = $derived(films.find((f) => f.name === recipe.film)?.is_bw ?? false);

  /**
   * @param {any} group
   * @param {any} control
   */
  function isControlDisabled(group, control) {
    if (control.id === "development_time_min" && !isBw) return true;
    if (!isPositive) return false;
    if (control.id === "paper") return true;
    if (group.label === "Tirages") return true;
    return false;
  }

  /**
   * @param {string} id
   */
  function isSubParam(id) {
    return (
      id === "glare_percent" || id === "glare_roughness" || id === "glare_blur" ||
      id === "preflash_y_shift" || id === "preflash_m_shift" ||
      id === "dir_couplers_amount" || id === "dir_couplers_diffusion_size" ||
      id === "dir_couplers_diffusion_tail" || id === "dir_couplers_tail_weight"
    );
  }

  /** A sub-param row is disabled when ITS OWN parent toggle is off — three
   * independent families share the row-graying mechanism (glare needs
   * recipe.glare, preflash's Y/M shifts need an actual preflash exposure,
   * DIR-coupler tuning needs the coupler itself active).
   * @param {string} id
   */
  function isSubParamDisabled(id) {
    if (id === "glare_percent" || id === "glare_roughness" || id === "glare_blur") {
      return !recipe.glare;
    }
    if (id === "preflash_y_shift" || id === "preflash_m_shift") {
      return !(recipe.preflash_exposure > 0);
    }
    if (
      id === "dir_couplers_amount" || id === "dir_couplers_diffusion_size" ||
      id === "dir_couplers_diffusion_tail" || id === "dir_couplers_tail_weight"
    ) {
      return !recipe.dir_couplers_active;
    }
    return false;
  }

  /**
   * @param {string} id
   * @param {number | undefined | null} v
   */
  function formatVal(id, v) {
    if (v === undefined || v === null) return "0";
    if (id === "temperature") {
      const kelvin = v <= 0 ? 5500 + v * 35 : 5500 + v * 45;
      return `${Math.round(kelvin)} K`;
    }
    if (id === "exposure_ev" || id === "vignette_amount") {
      return (v > 0 ? "+" : "") + v.toFixed(2);
    }
    if (
      id === "contrast" ||
      id === "saturation" ||
      id === "density_gamma" ||
      id === "zone_shadows_exposure" ||
      id === "zone_midtones_exposure" ||
      id === "zone_highlights_exposure" ||
      id === "zone_shadows_contrast" ||
      id === "zone_midtones_contrast" ||
      id === "zone_highlights_contrast"
    ) {
      return (v > 0 ? "+" : "") + v.toFixed(2);
    }
    if (
      id === "vignette_midpoint" ||
      id === "vignette_roundness" ||
      id === "vignette_feather" ||
      id === "grain_amount" ||
      id === "grain_roughness" ||
      id === "highlight_desat" ||
      id === "glare_percent" ||
      id === "glare_roughness" ||
      id === "glare_blur" ||
      id === "preflash_exposure" ||
      id === "dir_couplers_amount" ||
      id === "dir_couplers_tail_weight"
    ) {
      return v.toFixed(2);
    }
    if (id === "development_time_min") {
      return v <= 0 ? "Auto" : `${v.toFixed(1)} min`;
    }
    if (id === "film_format_mm") {
      return `${Math.round(v)} mm`;
    }
    return (v > 0 ? "+" : "") + Math.round(v);
  }
</script>

{#if engine && engine.control_groups}
  <div class="engine-runner">
    {#each engine.control_groups as group}
      {#if group.label}
        <button
          type="button"
          class="group-header"
          onclick={() => toggleGroup(group.label)}
          aria-expanded={!collapsedGroups.has(group.label)}
        >
          <span class="chevron" class:collapsed={collapsedGroups.has(group.label)}>
            <Icon name="caret-down" size="8px" />
          </span>
          <span class="group-title">{group.label}</span>
          <span class="group-line"></span>
        </button>
      {/if}

      {#if !group.label || !collapsedGroups.has(group.label)}
        <div class="group-controls">
          {#each group.controls as control}
            {#if control.kind === "toggle"}
              <div class="frow" class:disabled={isControlDisabled(group, control)}>
                <span class="din frow-label" title={control.label}>{control.label}</span>
                <span class="spacer"></span>
                <input
                  type="checkbox"
                  role="switch"
                  aria-label={control.label}
                  checked={recipe[control.id]}
                  onchange={() => {
                    recipe[control.id] = !recipe[control.id];
                    edited();
                  }}
                />
              </div>

            {:else if control.kind === "slider"}
              <div
                class="frow"
                class:sub-param={isSubParam(control.id)}
                class:disabled={isControlDisabled(group, control) || isSubParamDisabled(control.id)}
              >
                <button
                  type="button"
                  class="din frow-label reset-label"
                  aria-label={`Reset ${control.label} to default`}
                  title={`${control.label} — double-click or press Enter/Space to reset`}
                  disabled={isControlDisabled(group, control) || isSubParamDisabled(control.id)}
                  onclick={(event) => { if (event.detail === 0) resetControl(control.id); }}
                  ondblclick={() => resetControl(control.id)}
                >{control.label}</button>
                <input
                  type="range"
                  aria-label={control.label}
                  disabled={isControlDisabled(group, control) || isSubParamDisabled(control.id)}
                  min={control.min}
                  max={control.max}
                  step={control.step}
                  value={toDisplay(control.id, recipe[control.id])}
                  style="--f: {pct(toDisplay(control.id, recipe[control.id]), control.min, control.max)}"
                  oninput={(e) => {
                    recipe[control.id] = fromDisplay(control.id, parseFloat(e.currentTarget.value));
                    edited(true); // live proxy
                  }}
                  onchange={(e) => {
                    recipe[control.id] = fromDisplay(control.id, parseFloat(e.currentTarget.value));
                    edited(false); // full render
                  }}
                  ondblclick={() => resetControl(control.id)}
                />
                <span class="val mono">{formatVal(control.id, toDisplay(control.id, recipe[control.id]))}</span>
              </div>

            {:else if control.kind === "indexed_slider"}
              <div class="frow" class:disabled={isControlDisabled(group, control)}>
                <button
                  type="button"
                  class="din frow-label reset-label"
                  aria-label={`Reset ${control.label} to default`}
                  title={`${control.label} — double-click or press Enter/Space to reset`}
                  disabled={isControlDisabled(group, control)}
                  onclick={(event) => { if (event.detail === 0) resetControl(control.id, control.index); }}
                  ondblclick={() => resetControl(control.id, control.index)}
                >{control.label}</button>
                <input
                  type="range"
                  aria-label={control.label}
                  disabled={isControlDisabled(group, control)}
                  min={control.min}
                  max={control.max}
                  step={control.step}
                  value={recipe[control.id]?.[control.index] ?? 0}
                  style="--f: {pct(recipe[control.id]?.[control.index], control.min, control.max)}"
                  oninput={(e) => {
                    if (!Array.isArray(recipe[control.id])) recipe[control.id] = [];
                    recipe[control.id][control.index] = parseFloat(e.currentTarget.value);
                    edited(true);
                  }}
                  onchange={(e) => {
                    if (!Array.isArray(recipe[control.id])) recipe[control.id] = [];
                    recipe[control.id][control.index] = parseFloat(e.currentTarget.value);
                    edited(false);
                  }}
                  ondblclick={() => resetControl(control.id, control.index)}
                />
                <span class="val mono">{formatVal(control.id, recipe[control.id]?.[control.index])}</span>
              </div>

            {:else if control.kind === "select"}
              <div class="frow" class:disabled={isControlDisabled(group, control)}>
                <span class="din frow-label" title={control.label}>{control.label}</span>
                <span class="pick">
                  <select
                    value={recipe[control.id]}
                    onchange={(e) => {
                      recipe[control.id] = e.currentTarget.value;
                      edited();
                    }}
                  >
                    {#if control.options_type === "films"}
                      {#each films as f}
                        <option value={f.name}>{f.label}</option>
                      {/each}
                    {:else if control.options_type === "papers"}
                      {#each papers as p}
                        <option value={p.name}>{p.label}</option>
                      {/each}
                    {:else if control.options_type === "agx_looks"}
                      <option value="base">Base Contrast (Standard)</option>
                      <option value="punchy">Punchy (Éclatant)</option>
                      <option value="golden">Golden (Heure Dorée)</option>
                      <option value="soft">Soft (Doux)</option>
                      <option value="bw">Filmic B&W (Noir & Blanc)</option>
                    {/if}
                  </select>
                  <Icon name="caret-down" size="7px" />
                </span>
              </div>

            {:else if control.kind === "lut_stack"}
              <div class="lut-stack-section">
                <div class="frow sub-bar">
                  <span class="lut-subhead">
                    {control.stage === "pre" ? "Pre-Lut" : "Post-Lut"}
                  </span>
                  <span class="spacer"></span>
                  <button type="button" class="pill-btn add-lut-btn" onclick={() => addLutLayer(control.stage)}>
                    + LUT
                  </button>
                </div>

                {#if lutListFor(control.stage).length > 0}
                  {#each lutListFor(control.stage) as layer, idx}
                    <div class="lut-layer-card">
                      <div class="frow layer-row">
                        <span class="pick lut-file-pick">
                          <select
                            value={layer.name}
                            onchange={(e) => setLutFile(control.stage, idx, e.currentTarget.value)}
                          >
                            <option value="">(Aucun)</option>
                            {#each luts as name}
                              <option value={name}>{name}</option>
                            {/each}
                          </select>
                          <Icon name="caret-down" size="7px" />
                        </span>
                        <button
                          type="button"
                          class="icon-btn remove-lut-btn"
                          onclick={() => removeLutLayer(control.stage, idx)}
                          title="Supprimer cette couche LUT"
                        >
                          <Icon name="x" size="9px" />
                        </button>
                      </div>
                      {#if layer.name}
                        <div class="frow opacity-row">
                          <span class="din opacity-label">Opacité</span>
                          <span class="spacer"></span>
                          <input
                            type="range"
                            min="0"
                            max="1"
                            step="0.01"
                            value={layer.opacity}
                            style="--f: {pct(layer.opacity, 0, 1)}"
                            oninput={(e) =>
                              updateLutOpacity(control.stage, idx, parseFloat(e.currentTarget.value))}
                          />
                          <span class="val mono">{Math.round(layer.opacity * 100)}%</span>
                        </div>
                      {/if}
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .engine-runner {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }

  /* Precision Leica Collapsible Group Header */
  .group-header {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    margin-top: 10px;
    margin-bottom: 3px;
    padding: 2px 0;
    user-select: none;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
    transition: color var(--duration-fast) ease;
  }
  .group-header:hover {
    color: var(--color-foreground);
  }
  .group-header .chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: color-mix(in srgb, var(--color-foreground) 35%, transparent);
    transition: transform var(--duration-fast) ease, color var(--duration-fast) ease;
  }
  .group-header:hover .chevron {
    color: var(--color-foreground);
  }
  .group-header .chevron.collapsed {
    transform: rotate(-90deg);
  }
  .group-title {
    font-family: var(--font-header, sans-serif);
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    white-space: nowrap;
  }
  .group-line {
    flex: 1;
    height: 1px;
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
  }

  .group-controls {
    display: flex;
    flex-direction: column;
    gap: 1.5px;
  }

  /* Form Rows */
  .frow {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 20px;
    padding: 1px 0;
    transition: opacity var(--duration-fast);
  }
  .frow.sub-param {
    padding-left: 10px;
  }
  .frow.disabled {
    opacity: 0.22;
    pointer-events: none;
    filter: grayscale(1);
  }
  .frow-label {
    width: 90px;
    flex-shrink: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 60%, transparent);
    transition: color var(--duration-fast);
    cursor: default;
    text-align: right;
  }
  .reset-label {
    -webkit-appearance: none;
    appearance: none;
    background: none;
    border: 0;
    border-radius: 0;
    padding: 0;
    outline: none;
    box-shadow: none;
    text-align: right;
  }
  .reset-label:focus-visible {
    outline: 2px solid var(--color-accent);
    outline-offset: 2px;
  }
  .frow:hover .frow-label {
    color: var(--color-foreground);
  }
  .spacer {
    flex: 1;
  }
  .val {
    width: 44px;
    flex-shrink: 0;
    font-family: var(--font-monospace, monospace);
    font-size: 10px;
    text-align: right;
    color: color-mix(in srgb, var(--color-foreground) 80%, transparent);
    font-variant-numeric: tabular-nums;
  }

  /* Slider Styling — Precision Hairline Instrument */
  :global(input[type="range"]) {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    min-width: 0;
    height: 12px;
    background: transparent;
    margin: 0;
    cursor: pointer;
  }
  :global(input[type="range"]::-webkit-slider-runnable-track) {
    height: 1.5px;
    border-radius: 1px;
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--color-foreground) 55%, transparent) var(--f, 50%),
      color-mix(in srgb, var(--color-foreground) 12%, transparent) var(--f, 50%)
    );
  }
  :global(input[type="range"]::-webkit-slider-thumb) {
    -webkit-appearance: none;
    width: 6.5px;
    height: 6.5px;
    border-radius: 50%;
    background: var(--color-foreground);
    margin-top: -2.5px;
    border: none;
    box-shadow: 0 0.5px 2px rgba(0, 0, 0, 0.4);
    transition: transform var(--duration-instant) ease, background var(--duration-instant) ease;
  }
  :global(input[type="range"]:hover::-webkit-slider-thumb) {
    transform: scale(1.25);
  }
  :global(input[type="range"]:active::-webkit-slider-thumb) {
    background: var(--color-accent, #d6202c);
  }

  /* Select Pickers */
  .pick {
    flex: 1;
    display: flex;
    align-items: center;
    position: relative;
  }
  .pick select {
    width: 100%;
    box-sizing: border-box;
    font-family: var(--font-text, sans-serif);
    font-size: 10.5px;
    background: color-mix(in srgb, var(--color-foreground) 3%, transparent);
    background-image: none;
    color: var(--color-foreground, #fff);
    border: 1px solid color-mix(in srgb, var(--color-foreground) 12%, transparent);
    border-radius: var(--radius-sm, 3px);
    padding: 2.5px 18px 2.5px 6px;
    outline: none;
    box-shadow: none;
    appearance: none;
    -webkit-appearance: none;
    transition: border-color var(--duration-fast), background var(--duration-fast);
  }
  .pick select:hover {
    background: color-mix(in srgb, var(--color-foreground) 6%, transparent);
    border-color: color-mix(in srgb, var(--color-foreground) 18%, transparent);
  }
  .pick select:focus {
    border-color: var(--color-accent);
  }
  .pick :global(.icon) {
    position: absolute;
    right: 6px;
    pointer-events: none;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
  }

  /* LUT Stacks */
  .lut-stack-section {
    margin-top: 0.3rem;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  .sub-bar {
    display: flex;
    align-items: center;
  }
  .lut-subhead {
    font-family: var(--font-header, sans-serif);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 45%, transparent);
  }
  .pill-btn {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--color-foreground) 12%, transparent);
    background: color-mix(in srgb, var(--color-foreground) 3%, transparent);
    color: var(--color-foreground);
    transition: background 120ms, border-color 120ms;
  }
  .pill-btn:hover {
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    border-color: var(--color-accent);
  }

  .lut-layer-card {
    background: color-mix(in srgb, var(--color-foreground) 2.5%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground) 7%, transparent);
    border-radius: var(--radius-sm, 3px);
    padding: 4px 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .layer-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .icon-btn {
    all: unset;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    color: color-mix(in srgb, var(--color-foreground) 40%, transparent);
    transition: color 120ms, background 120ms;
  }
  .icon-btn:hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 10%, transparent);
  }
  .opacity-label {
    font-size: 9.5px;
  }
  .mono {
    font-family: var(--font-monospace, monospace);
  }
</style>
