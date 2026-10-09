<script>
  import { Icon } from "@modules/core";

  // One LUT stack (pre- OR post-engine) — extracted from EngineRunner so it can
  // live in the dedicated LUT palette. Rapid uses `rapid_pre_luts` /
  // `rapid_post_luts`; deprecated global keys are still read and migrated for
  // older sidecars. `luts` = .cube filename strings stripped to display names.
  /** @typedef {{ name: string, opacity: number }} LutLayer */
  /** @typedef {{ engine?: string, rapid_pre_luts?: LutLayer[], rapid_post_luts?: LutLayer[], pre_luts?: LutLayer[], post_luts?: LutLayer[] }} LutRecipe */

  /** @type {{ stage?: string, recipe: LutRecipe, luts: (string|LutLayer)[], edited: function(boolean=): void }} */
  let {
    stage = "pre",
    recipe = $bindable(),
    luts = [],
    edited = () => {},
  } = $props();

  const key = $derived(recipe?.engine === "rapid"
    ? (stage === "pre" ? "rapid_pre_luts" : "rapid_post_luts")
    : (stage === "pre" ? "pre_luts" : "post_luts"));
  const oldKey = $derived(stage === "pre" ? "pre_luts" : "post_luts");
  const layers = $derived(
    recipe
      ? ((recipe[key] && recipe[key].length > 0)
          ? recipe[key]
          : (recipe.engine === "rapid" ? (recipe[oldKey] || []) : (recipe[key] || [])))
      : []
  );

  function ensureMigration() {
    if (!recipe || recipe.engine !== "rapid" || !recipe[oldKey]?.length) return;
    recipe[key] = [...(recipe[key] ?? []), ...recipe[oldKey]];
    recipe[oldKey] = [];
    edited();
  }

  function addLayer() {
    if (!recipe) return;
    ensureMigration();
    const first = typeof luts[0] === "string" ? luts[0] : (luts[0]?.name ?? "");
    recipe[key] = [...(recipe[key] ?? []), { name: first, opacity: 1 }];
    edited();
  }
  /** @param {number} index */
  function removeLayer(index) {
    if (!recipe) return;
    ensureMigration();
    recipe[key] = (recipe[key] ?? []).filter((_, i) => i !== index);
    edited();
  }
  /**
   * @param {number} index
   * @param {number} value
   */
  function updateOpacity(index, value) {
    if (!recipe) return;
    ensureMigration();
    recipe[key] = (recipe[key] ?? []).map((l, i) =>
      i === index ? { ...l, opacity: Number(value) } : l,
    );
    edited(true);
  }
  /**
   * @param {number} index
   * @param {string} name
   */
  function setFile(index, name) {
    if (!recipe) return;
    ensureMigration();
    recipe[key] = (recipe[key] ?? []).map((l, i) => (i === index ? { ...l, name } : l));
    edited();
  }
</script>

  <div class="lut-stack-section">
    <div class="frow sub-bar">
      <span class="din frow-label">
        {stage === "pre" ? "Pre-Lut" : "Post-Lut"}
      </span>
      <span class="spacer"></span>
      <button class="ghost small add-lut-btn" onclick={addLayer}>+ LUT</button>
    </div>

    {#each layers as layer, idx}
      <div class="frow lut-row">
        <select
          class="panel-select lut-file-pick"
          value={layer.name}
          title={layer.name || "Select a LUT"}
          onchange={(e) => setFile(idx, e.currentTarget.value)}
        >
          <option value="">(None)</option>
          {#each luts as opt}
            {@const val = typeof opt === "string" ? opt : opt.name}
            <option value={val}>{val}</option>
          {/each}
        </select>
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={layer.opacity}
          style="--slider-value: {layer.opacity * 100}%"
          title={`Opacity: ${Math.round(layer.opacity * 100)}%`}
          oninput={(e) => updateOpacity(idx, parseFloat(e.currentTarget.value))}
        />
        <button
          type="button"
          class="remove-lut-btn"
          onclick={() => removeLayer(idx)}
          title={`Opacity: ${Math.round(layer.opacity * 100)}% — click to remove`}
          aria-label="Remove LUT"
        >
          <span class="lut-val mono">{Math.round(layer.opacity * 100)}%</span>
          <span class="lut-del"><Icon name="x" size="var(--icon-sm)" /></span>
        </button>
      </div>
    {/each}
</div>

<style>
  .lut-stack-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
  }
  .frow {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    min-height: 22px;
  }
  .sub-bar {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    min-height: 22px;
  }
  .frow-label {
    width: 7.0rem;
    flex-shrink: 0;
    font-size: 0.76rem;
    color: var(--color-foreground);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-interface);
  }
  .spacer {
    flex: 1;
  }
  .add-lut-btn {
    font-size: 0.68rem;
    padding: 1px 6px;
    height: 18px;
    line-height: 16px;
    color: var(--color-muted);
    background: transparent;
    border: none;
    box-shadow: none;
    cursor: pointer;
    border-radius: var(--radius);
    transition: color var(--transition-fast), background-color var(--transition-fast);
  }
  .add-lut-btn:hover {
    color: var(--color-foreground);
    background-color: var(--color-hover);
  }
  .lut-row {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    min-height: 22px;
  }
  .lut-file-pick {
    width: 7.0rem;
    flex-shrink: 0;
    height: 22px;
    font-size: 0.72rem;
    background: var(--color-surface);
    box-shadow: none;
    border-radius: var(--radius);
    color: var(--color-foreground);
    padding: 0 var(--space-d2);
    text-overflow: ellipsis;
    white-space: nowrap;
    overflow: hidden;
    cursor: pointer;
  }
  .lut-row input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .remove-lut-btn {
    appearance: none;
    -webkit-appearance: none;
    width: 3.2rem;
    flex-shrink: 0;
    height: 22px;
    padding: 0 2px !important;
    margin: 0 !important;
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    background: transparent !important;
    border: none !important;
    box-shadow: none !important;
    border-radius: var(--radius);
    cursor: pointer;
    font-family: inherit;
    font-size: 0.72rem;
    color: var(--color-muted);
  }
  .lut-val {
    font-size: 0.72rem;
    color: var(--color-muted);
    text-align: right;
    line-height: 1;
  }
  .lut-del {
    display: none;
    align-items: center;
    justify-content: center;
    color: var(--color-muted);
    transition: color var(--transition-fast);
  }
  .lut-row:hover .lut-val {
    display: none;
  }
  .lut-row:hover .lut-del {
    display: inline-flex;
  }
  .lut-row:has(input[type="range"]:active) .lut-val {
    display: inline-block !important;
  }
  .lut-row:has(input[type="range"]:active) .lut-del {
    display: none !important;
  }
  .remove-lut-btn:hover .lut-del {
    color: var(--color-accent);
  }
</style>
