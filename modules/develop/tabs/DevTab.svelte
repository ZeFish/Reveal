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

      <button class="capsule outline mt" onclick={resetRecipe}>Réinitialiser</button>
    {/if}
    </div>
  </div>
</div>

<style>
  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .sec-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
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
    gap: 12px;
  }
  .engine-row-label {
    flex-shrink: 0;
  }
  .seg {
    display: flex;
    background: color-mix(in srgb, var(--color-foreground) 4%, transparent);
    border: 1px solid var(--color-border);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .seg-btn {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 4px 10px;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
    background: transparent;
    transition: color var(--duration-fast), background var(--duration-fast);
  }
  .seg-btn.on {
    color: var(--color-background);
    background: var(--color-foreground);
  }
  .seg-btn:not(.on):hover {
    color: var(--color-foreground);
    background: color-mix(in srgb, var(--color-foreground) 8%, transparent);
  }

  .engine-scope {
    transition: opacity var(--duration-fast);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .engine-scope.inactive {
    opacity: 0.4;
    pointer-events: none;
  }

  .capsule {
    all: unset;
    display: block;
    width: 100%;
    box-sizing: border-box;
    cursor: pointer;
    text-align: center;
    padding: 8px 0;
    border-radius: 999px;
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .capsule.outline {
    border: 1px solid var(--color-border);
    color: color-mix(in srgb, var(--color-foreground) 85%, transparent);
  }
  .mt {
    margin-top: 14px;
  }
</style>
