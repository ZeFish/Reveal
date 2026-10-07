<script>
  import EngineRunner from "@modules/develop/EngineRunner.svelte";
  import CollapsibleGroup from "@modules/develop/engines/controls/CollapsibleGroup.svelte";
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
    activeZone = $bindable("global"),
    onSetZoneMask = () => {},
  } = $props();

  /** @param {string} id */
  function handleEngineChanged(id) {
    if (id !== "rapid" && activeZone !== "global") {
      activeZone = "global";
      onSetZoneMask(null);
    }
    engineChanged(id);
  }
</script>

<div class="dev-tab">
  <!-- Stays put: the scopes, the photo size and the engine switch. The zone tabs
       (Rapid) are stuck to the top of the scrolling part below, so the whole
       header — scopes to zones — is always in reach while the controls move. -->
  <div class="sec-body dev-head">
    <CollapsibleGroup label="Histogram">
      <Scopes {histogram} {scopes} />
    </CollapsibleGroup>

    <div class="engine-row">
      <div class="btn-group" role="group" aria-label="Engine">
        <button
          type="button"
          aria-pressed={!developEngine}
          onclick={() => handleEngineChanged("none")}
        >None</button>
        {#each engines as e}
          <button
            type="button"
            class="small"
            aria-pressed={developEngine === e.id}
            onclick={() => handleEngineChanged(e.id)}
          >{e.label}</button>
        {/each}
      </div>
    </div>

  </div>

  <div class="pane-scroll">
    <div class="engine-scope" class:inactive={!developEngine}>
      {#if activeEngine}
        <EngineRunner
          engine={activeEngine}
          bind:recipe={recipe}
          bind:activeZone={activeZone}
          {films}
          {papers}
          {luts}
          {edited}
          resetControl={resetOne}
          {addLutLayer}
          {removeLutLayer}
          {updateLutOpacity}
          {setLutFile}
          {resetRecipe}
          {onSetZoneMask}
        />

        <div class="btn-row footer">
          <button class="outline panel-btn half" onclick={resetRecipe}>
            Reset
          </button>
          <button class="accent panel-btn half" onclick={() => onExport()} disabled={!photoPath}>Export</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .dev-tab {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 0;
    font-size: 11px;
  }

  /* The head is a stack of bars (scopes, engine switch, zones), each with the same air above
     and below and a hairline under it: the rhythm comes from the bars, not from a gap. */
  .dev-head {
    flex-shrink: 0;
    gap: 0;
  }

  .pane-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* No scrollbar: it sat on top of the values. The trackpad and the wheel scroll as before. */
    scrollbar-width: none;
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 3);
  }


  .sec-body {
    display: flex;
    flex-direction: column;
    gap: 0;
  }


  /* Segmented control for the engine switch */
  .engine-row {
    padding: 0;
  }
  /* The switch spans the panel; its buttons share the width equally. */
  .engine-row .btn-group {
    display: flex;
    width: 100%;
  }
  .engine-row .btn-group > button {
    min-height: var(--control-h);
    padding-block: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .engine-scope {
    transition: opacity var(--duration-fast);
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
    /* At least the height of the scrolling area, so the Reset / Export row below can sit at the
       bottom of the panel even when the controls are few (Spektra). Longer content simply
       scrolls, and the row stays pinned (sticky) over it. */
    flex: 1 0 auto;
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
  /* Reset / Export stay in reach: pinned to the bottom of the scrolling area. */
  .footer {
    margin-top: auto;
    position: sticky;
    bottom: 0;
    z-index: 5;
    padding: var(--space-d3) 0;
    background: var(--color-surface-light-1);
    border-top: var(--border);
  }
</style>
