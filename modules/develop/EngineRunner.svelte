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

  function toggleGroup(label) {
    const next = new Set(collapsedGroups);
    if (next.has(label)) next.delete(label);
    else next.add(label);
    collapsedGroups = next;
    try {
      localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...next]));
    } catch {}
  }

  function lutListFor(stage) {
    if (stage === "pre") return recipe.rapid_pre_luts?.length ? recipe.rapid_pre_luts : (recipe.pre_luts || []);
    return recipe.rapid_post_luts?.length ? recipe.rapid_post_luts : (recipe.post_luts || []);
  }

  const pct = (v, min, max) => `${((Number(v ?? 0) - min) / (max - min)) * 100}%`;

  let isPositive = $derived(
    films.find((f) => f.name === recipe.film)?.film_type === "positive"
  );

  function isControlDisabled(group, control) {
    if (!isPositive) return false;
    if (control.id === "paper") return true;
    if (group.label === "Tirages") return true;
    return false;
  }

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
      id === "glare_blur"
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
          class="din group-label group-toggle"
          onclick={() => toggleGroup(group.label)}
          aria-expanded={!collapsedGroups.has(group.label)}
        >
          <span class="chevron">{collapsedGroups.has(group.label) ? "▶" : "▼"}</span>
          {group.label}
        </button>
      {/if}

      {#if !group.label || !collapsedGroups.has(group.label)}
      {#each group.controls as control}
        {#if control.kind === "toggle"}
          <div class="frow" class:disabled={isControlDisabled(group, control)}>
            <span class="din frow-label">{control.label}</span>
            <span class="spacer"></span>
            <button
              class="toggle"
              class:on={recipe[control.id]}
              role="switch"
              aria-label={control.label}
              aria-checked={recipe[control.id]}
              onclick={() => {
                recipe[control.id] = !recipe[control.id];
                edited();
              }}
            >
              <span class="knob"></span>
            </button>
          </div>

        {:else if control.kind === "slider"}
          <div class="frow" class:disabled={isControlDisabled(group, control)}>
            <span
              class="din frow-label"
              title="Double-clic : remettre au défaut"
              ondblclick={() => resetControl(control.id)}
            >{control.label}</span>
            <input
              type="range"
              min={control.min}
              max={control.max}
              step={control.step}
              value={recipe[control.id]}
              style="--f: {pct(recipe[control.id], control.min, control.max)}"
              oninput={(e) => {
                recipe[control.id] = parseFloat(e.currentTarget.value);
                edited(true); // live: renders the small DRAG_PX proxy, snappy
              }}
              onchange={(e) => {
                recipe[control.id] = parseFloat(e.currentTarget.value);
                edited(false); // settle: one full-resolution render on release
              }}
              ondblclick={() => resetControl(control.id)}
            />
            <span class="val">{formatVal(control.id, recipe[control.id])}</span>
          </div>

        {:else if control.kind === "indexed_slider"}
          <div class="frow" class:disabled={isControlDisabled(group, control)}>
            <span
              class="din frow-label"
              title="Double-clic : remettre au défaut"
              ondblclick={() => resetControl(control.id, control.index)}
            >{control.label}</span>
            <input
              type="range"
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
            <span class="val">{formatVal(control.id, recipe[control.id]?.[control.index])}</span>
          </div>

        {:else if control.kind === "select"}
          <div class="frow" class:disabled={isControlDisabled(group, control)}>
            <span class="din frow-label">{control.label}</span>
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
              <Icon name="caret-down" size="8px" />
            </span>
          </div>

        {:else if control.kind === "lut_stack"}
          <div class="lut-stack-section">
            <div class="frow sub-bar">
              <span class="lut-subhead"
                >{control.stage === "pre" ? "Pre-Lut" : "Post-Lut"}</span
              >
              <span class="spacer"></span>
              <button class="pill-btn add-lut-btn" onclick={() => addLutLayer(control.stage)}>
                + LUT
              </button>
            </div>

            {#if lutListFor(control.stage).length === 0}

            {:else}
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
                      <Icon name="caret-down" size="8px" />
                    </span>
                    <button
                      class="icon-btn remove-lut-btn"
                      onclick={() => removeLutLayer(control.stage, idx)}
                      title="Supprimer cette couche LUT"
                    >
                      ×
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
                        oninput={(e) =>
                          updateLutOpacity(control.stage, idx, parseFloat(e.currentTarget.value))}
                      />
                      <span class="val">{Math.round(layer.opacity * 100)}%</span>
                    </div>
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        {/if}
      {/each}
      {/if}
    {/each}
  </div>
{/if}

<style>
  .engine-runner {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .group-label {
    font-family: var(--font-monospace, monospace);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    opacity: 0.75;
    margin-top: 0.75rem;
    margin-bottom: 0.35rem;
  }
  .group-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    border: none;
    background: transparent;
    padding: 0;
    color: inherit;
    cursor: pointer;
    text-align: left;
  }
  .group-toggle .chevron {
    display: inline-block;
    width: 0.8em;
    font-size: 0.65em;
    opacity: 0.6;
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 16px;
    transition: opacity var(--duration-standard);
  }
  .frow.disabled {
    opacity: 0.25;
    pointer-events: none;
    filter: grayscale(1);
  }
  .frow-label {
    width: 88px;
    flex-shrink: 0;
    white-space: nowrap;
    font-family: var(--font-monospace, monospace);
    font-size: 0.6rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    opacity: 0.7;
  }
  .spacer {
    flex: 1;
  }
  .val {
    min-width: 48px;
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    text-align: right;
    opacity: 0.8;
    font-variant-numeric: tabular-nums;
  }
  /* Foreground-tinted slider — the track fills to `--f` (set inline per
     value); matches the original dev-panel sliders. */
  :global(input[type="range"]) {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    min-width: 0;
    height: 12px;
    background: transparent;
    margin: 0;
  }
  :global(input[type="range"]::-webkit-slider-runnable-track) {
    height: 3px;
    border-radius: var(--radius-sm);
    background: linear-gradient(
      to right,
      var(--color-foreground) var(--f, 50%),
      var(--color-border) var(--f, 50%)
    );
  }
  :global(input[type="range"]::-webkit-slider-thumb) {
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-foreground);
    margin-top: -4.5px;
    border: none;
  }
  .pick {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .pick select {
    width: 100%;
    font-family: var(--font-text, sans-serif);
    font-size: 0.75rem;
    background: var(--color-surface-low, #1e1e1e);
    color: var(--color-foreground, #fff);
    border: 1px solid var(--color-border, #333);
    border-radius: var(--radius-sm);
    padding: 2px 6px;
  }
  .toggle {
    flex-shrink: 0;
    width: 28px;
    height: 16px;
    border-radius: 8px;
    background: var(--color-surface-high, #333);
    border: none;
    position: relative;
    cursor: pointer;
    padding: 0;
    transition: background var(--duration-fast);
  }
  .toggle.on {
    background: var(--color-accent);
  }
  .toggle .knob {
    display: block;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    position: absolute;
    top: 2px;
    left: 2px;
    transition: transform var(--duration-fast);
  }
  .toggle.on .knob {
    transform: translateX(12px);
  }
  .lut-stack-section {
    margin-top: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .sub-bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .lut-subhead {
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    letter-spacing: 0.05em;
    opacity: 0.85;
    text-transform: uppercase;
  }
  .lut-hint {
    font-size: 0.68rem;
    opacity: 0.6;
    margin: 0.2rem 0;
    line-height: 1.3;
  }
  .lut-layer-card {
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid var(--color-border, #333);
    border-radius: var(--radius);
    padding: 0.4rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .layer-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .lut-file-pick {
    flex: 1;
  }
  .icon-btn, .pill-btn {
    font-size: 0.65rem;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border, #333);
    background: var(--color-surface-low, #1e1e1e);
    color: var(--color-foreground, #fff);
    cursor: pointer;
  }
</style>
