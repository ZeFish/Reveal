<script>
  import EngineRunner from "@modules/develop/EngineRunner.svelte";
  import { DEFAULT_PHOTO_SIZE, PHOTO_SIZE_MIN, PHOTO_SIZE_MAX } from "$lib/session.js";
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
    photoScale = $bindable(DEFAULT_PHOTO_SIZE),
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
        min={PHOTO_SIZE_MIN}
        max={PHOTO_SIZE_MAX}
        step="5"
        value={photoScale}
        style="--slider-value: {((photoScale - PHOTO_SIZE_MIN) / (PHOTO_SIZE_MAX - PHOTO_SIZE_MIN)) * 100}%"
        oninput={(e) => {
          photoScale = Number(e.currentTarget.value);
          onPhotoScaleChanged(photoScale);
        }}
      />
      <span class="val mono">{photoScale}%</span>
    </div>
    <div class="engine-row">
      <div class="btn-group" role="group" aria-label="Engine">
        <button
          type="button"
          aria-pressed={!developEngine}
          onclick={() => engineChanged("none")}
        >None</button>
        {#each engines as e}
          <button
            type="button"
            aria-pressed={developEngine === e.id}
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
    gap: var(--space-d2);
  }

  .frow {
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    padding-bottom: var(--space-d3);
    border-bottom: var(--stroke-width) solid color-mix(in srgb, var(--color-foreground) 8%, transparent);
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
    min-width: 36px;
    white-space: nowrap;
    flex-shrink: 0;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* Segmented control for the engine switch */
  .engine-row {
    padding-bottom: var(--space-d3);
    border-bottom: var(--stroke-width) solid color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .engine-scope {
    transition: opacity var(--duration-fast);
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
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
    padding: var(--space-d3) 0;
  }
  .btn-row {
    display: flex;
    gap: var(--space-d2);
  }
  .half {
    flex: 1;
    width: auto;
  }
  .mt {
    margin-top: var(--space);
  }
</style>
