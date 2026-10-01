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
    <span class="din empty">No selection</span>
  {/if}
</div>

<style>

  .hairline-inner {
    height: 1px;
    background: var(--color-border);
    opacity: 0.6;
    margin: var(--space-d4) 0;
  }

  .pane-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space);
  }

  .empty {
    padding: calc(var(--space-d4) * 6) 0;
    text-align: center;
  }
</style>
