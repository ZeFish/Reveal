<script>
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
      <span class="lut-subhead">
        {stage === "pre" ? "Pre-Lut" : "Post-Lut"}
      </span>
      <span class="spacer"></span>
      <button class="outline small add-lut-btn" onclick={addLayer}>+ LUT</button>
    </div>

    {#if layers.length === 0}

    {:else}
      {#each layers as layer, idx}
      <div class="lut-layer-card">
        <div class="frow layer-row">
          <select class="lut-file-pick" value={layer.name} onchange={(e) => setFile(idx, e.currentTarget.value)}>
            <option value="">(Aucun)</option>
            {#each luts as name}
              <option value={name}>{name}</option>
            {/each}
          </select>
          <button
            class="ghost icon-btn remove-lut-btn"
            onclick={() => removeLayer(idx)}
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
              style="--slider-value: {layer.opacity * 100}%"
              oninput={(e) => updateOpacity(idx, parseFloat(e.currentTarget.value))}
            />
            <span class="val">{Math.round(layer.opacity * 100)}%</span>
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .lut-stack-section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .frow {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 16px;
  }
  .sub-bar {
    gap: 0.5rem;
  }
  .spacer {
    flex: 1;
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
    gap: 0.4rem;
  }
  .din {
    font-family: var(--font-monospace, monospace);
    font-size: 0.6rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    opacity: 0.7;
  }
  .val {
    min-width: 30px;
    font-family: var(--font-monospace, monospace);
    font-size: 0.62rem;
    text-align: right;
    opacity: 0.8;
    font-variant-numeric: tabular-nums;
  }
  /* Sizing only from here down — select/button/range-slider identity
     (chevron, borders, hover, fill/thumb) all come from Standard's own
     zero-class rules (_standard-11-forms.scss, _standard-13-components.scss)
     plus the app-wide pill shape in +layout.svelte. --slider-value (set
     inline per input above) is what drives the slider's fill gradient. */
  .lut-file-pick {
    flex: 1;
    font-size: 0.75rem;
    padding: 2px 6px;
  }
  input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .icon-btn,
  .add-lut-btn {
    font-size: 0.65rem;
    padding: 2px 6px;
  }
</style>
