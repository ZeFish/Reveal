<script>
  import EngineRunner from "@modules/develop/EngineRunner.svelte";
  import Scopes from "@modules/develop/Scopes.svelte";

  let {
    recipe = $bindable(),
    engines = [],
    developEngine,
    activeEngine,
    films = [],
    papers = [],
    luts = [],
    edited,
    resetOne,
    addLutLayer,
    removeLutLayer,
    updateLutOpacity,
    setLutFile,
    engineChanged,
    resetRecipe,
    onExport = () => {},
    photoPath = null,
    histogram = null,
    scopes = null,
    // The "frame" zoom's size as a % of the viewport — a plain view setting,
    // not part of the per-photo recipe. >100 lets it outgrow the frame; past
    // that point dragging the photo pans it instead of opening the loupe
    // (see DevelopView.svelte / onPhotoPointerDown in +page.svelte).
    photoScale = $bindable(90),
    onPhotoScaleChanged = () => {},
  } = $props();
</script>

<div class="pane-scroll">
  <div class="sec-body">
    <Scopes {histogram} {scopes} />
    <div class="frow">
      <span class="din frow-label">Photo Size</span>
      <input
        type="range"
        min="40"
        max="200"
        step="5"
        value={photoScale}
        oninput={(e) => {
          photoScale = Number(e.currentTarget.value);
          onPhotoScaleChanged(photoScale);
        }}
      />
      <span class="val mono">{photoScale}%</span>
    </div>
    <div class="engine-row">
      <div class="seg" role="group" aria-label="Engine">
        <button
          type="button"
          class="seg-btn"
          class:on={!developEngine}
          onclick={() => engineChanged("none")}
        >None</button>
        {#each engines as e}
          <button
            type="button"
            class="seg-btn ghost"
            class:on={developEngine === e.id}
            onclick={() => engineChanged(e.id)}
          >{e.label}</button>
        {/each}
      </div>
    </div>
    <div class="engine-scope" class:inactive={!developEngine}>
    {#if activeEngine}
      <EngineRunner
        engine={activeEngine}
        bind:recipe={recipe}
        {films}
        {papers}
        {luts}
        {edited}
        resetControl={resetOne}
        {addLutLayer}
        {removeLutLayer}
        {updateLutOpacity}
        {setLutFile}
      />

      <div class="btn-row mt">
        <button class="outline panel-btn half" onclick={resetRecipe}>Reset</button>
        <button class="secondary panel-btn half" onclick={() => onExport()} disabled={!photoPath}>Export</button>
      </div>
    {/if}
    </div>
  </div>
</div>

<style>
  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 12px 14px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .sec-body {
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

  .frow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .frow-label {
    width: 90px;
    flex-shrink: 0;
    white-space: nowrap;
    text-align: right;
  }
  .frow input[type="range"] {
    flex: 1;
    min-width: 0;
  }
  .val {
    width: 36px;
    flex-shrink: 0;
    font-size: 10px;
    text-align: right;
    color: color-mix(in srgb, var(--color-foreground) 70%, transparent);
    font-variant-numeric: tabular-nums;
  }
  .mono {
    font-family: var(--font-monospace, monospace);
  }

  /* Segmented control for the engine switch */
  .engine-row {
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .engine-scope {
    transition: opacity var(--duration-fast);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .engine-scope.inactive {
    opacity: 0.35;
    pointer-events: none;
  }

  /* Sizing only — color/shape/identity come from Standard's own button
     rules (packages/styles/_standard-13-components.scss: plain button,
     .secondary, .outline) plus the app-wide pill shape in +layout.svelte.
     This panel is dense enough to need a smaller footprint than either
     provides by default. */
  .panel-btn {
    padding: 6px 0;
    font-size: 10px;
    letter-spacing: 0.1em;
  }
  .btn-row {
    display: flex;
    gap: 8px;
  }
  .half {
    flex: 1;
    width: auto;
  }
  .mt {
    margin-top: 16px;
  }
</style>
