<script>
  import { getEngineComponent } from "./engines/engineRegistry.js";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauri } from "@modules/core";

  let {
    engine,
    recipe = $bindable(),
    activeZone = $bindable("global"),
    films = [],
    papers = [],
    luts = [],
    edited = () => {},
    resetControl = () => {},
    addLutLayer = () => {},
    removeLutLayer = () => {},
    updateLutOpacity = () => {},
    setLutFile = () => {},
    resetRecipe = () => {},
    onSetZoneMask = () => {},
  } = $props();

  /** @type {any} */
  let defaults = $state(null);
  if (isTauri) {
    invoke("default_recipe").then((d) => (defaults = d)).catch(() => {});
  }

  let EngineComponent = $derived(getEngineComponent(engine?.id));
</script>

{#if engine}
  <div class="engine-runner" data-zone={activeZone}>
    <EngineComponent
      {engine}
      bind:recipe
      bind:activeZone
      {films}
      {papers}
      {luts}
      {defaults}
      {edited}
      {resetControl}
      {addLutLayer}
      {removeLutLayer}
      {updateLutOpacity}
      {setLutFile}
      {resetRecipe}
      {onSetZoneMask}
    />
  </div>
{/if}

<style>
  .engine-runner {
    display: flex;
    flex-direction: column;
    gap: var(--space-d4);
  }
</style>
