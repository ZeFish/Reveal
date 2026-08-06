<script>
  import LutStack from "@modules/develop/LutStack.svelte";

  let { recipe = $bindable(), luts, edited } = $props();
</script>

<div class="pane-scroll">
  {#if recipe}
    <LutStack stage="pre" bind:recipe {luts} {edited} />
    <div class="hairline-inner"></div>
    <LutStack stage="post" bind:recipe {luts} {edited} />
  {:else}
    <span class="din empty">Aucune sélection</span>
  {/if}
</div>

<style>
  .din {
    font-family: var(--font-header, sans-serif);
    font-size: 10.8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-foreground) 55%, transparent);
  }

  .hairline-inner {
    height: 1px;
    background: var(--color-border);
    opacity: 0.6;
    margin: 0.25rem 0;
  }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .empty {
    padding: 24px 0;
    text-align: center;
  }
</style>
