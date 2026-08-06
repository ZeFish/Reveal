<script>
  // App-local icon renderer over the inlined registry in $lib/icons.js —
  // see that file for why Reveal doesn't use @stnd/icon's fetching Icon.
  import { icons } from "$lib/icons.js";

  let { name, size = "13px", class: className = "" } = $props();

  const svg = $derived(icons[/** @type {keyof typeof icons} */ (name)] ?? "");
</script>

<span class="icon {className}" style="width:{size};height:{size}" aria-hidden="true">
  {@html svg}
</span>

<style>
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .icon :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
