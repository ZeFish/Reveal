<script>
  import EngineRunner from "@modules/develop/EngineRunner.svelte";

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
  } = $props();
</script>

<div class="pane-scroll">
  <div class="sec-body">
    <div class="engine-row">
      <span class="din engine-row-label">Moteur</span>
      <div class="seg">
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
        <button class="capsule outline half" onclick={resetRecipe}>Réinitialiser</button>
        <button class="capsule fill half" onclick={() => onExport()} disabled={!photoPath}>Export</button>
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

  /* Segmented control for the engine switch */
  .engine-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }
  .engine-row-label {
    font-weight: 600;
  }
  .seg {
    display: flex;
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-foreground) 10%, transparent);
    border-radius: var(--radius-sm, 4px);
    overflow: hidden;
    padding: 1px;
  }
  .seg-btn {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 3px 9px;
    border-radius: 3px;
    color: color-mix(in srgb, var(--color-foreground) 50%, transparent);
    background: transparent;
    transition: color var(--duration-fast), background var(--duration-fast);
  }
  .seg-btn.on {
    color: var(--color-background);
    background: var(--color-foreground);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
  }
  .seg-btn:not(.on):hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
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

  .capsule {
    all: unset;
    display: block;
    width: 100%;
    box-sizing: border-box;
    cursor: pointer;
    text-align: center;
    padding: 6px 0;
    border-radius: var(--radius-sm, 4px);
    font-family: var(--font-header, sans-serif);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    transition: background var(--duration-fast), border-color var(--duration-fast);
  }
  .capsule.outline {
    border: 1px solid color-mix(in srgb, var(--color-foreground) 12%, transparent);
    background: color-mix(in srgb, var(--color-foreground) 3%, transparent);
    color: color-mix(in srgb, var(--color-foreground) 75%, transparent);
  }
  .capsule.outline:hover {
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
    border-color: color-mix(in srgb, var(--color-foreground) 25%, transparent);
    color: var(--color-foreground);
  }
  .capsule.fill {
    color: var(--color-background);
    background: var(--color-foreground);
  }
  .capsule:disabled {
    opacity: 0.25;
    cursor: default;
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
