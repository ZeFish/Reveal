<script module>
  // Every Phosphor icon in @stnd/icon, each as its own lazily-loaded chunk.
  //
  // $lib/icons.js inlines the icons the app shows on first paint, so those
  // render with no wait. It used to be the ONLY source, and a name missing
  // from its hand-kept list rendered as nothing, silently: on 2026-09-23,
  // nine icons in use (the settings categories, crop flips, develop badges)
  // had never been added. Anything not inlined now loads from the package
  // itself, so an icon that exists cannot go missing.
  const lazy = import.meta.glob("../../../../../packages/icon/icons/ph/*.svg", {
    query: "?raw",
    import: "default",
  });
  /** @type {Map<string, Promise<string>>} */
  const loaded = new Map();

  /** @param {string} name @returns {Promise<string>} */
  function load(name) {
    let pending = loaded.get(name);
    if (!pending) {
      const loader = lazy[`../../../../../packages/icon/icons/ph/${name}.svg`];
      if (!loader) console.warn(`Icon "${name}" is not a Phosphor icon in packages/icon/icons/ph`);
      pending = loader ? /** @type {Promise<string>} */ (loader()) : Promise.resolve("");
      loaded.set(name, pending);
    }
    return pending;
  }
</script>

<script>
  // App-local icon renderer over the inlined registry in $lib/icons.js —
  // see that file for why Reveal doesn't use @stnd/icon's fetching Icon.
  import { icons } from "$lib/icons.js";

  let { name, size = "13px", class: className = "" } = $props();

  const inlined = $derived(icons[/** @type {keyof typeof icons} */ (name)]);
  let fetched = $state("");
  $effect(() => {
    if (inlined) return;
    const wanted = name;
    fetched = "";
    load(wanted).then((svg) => {
      if (name === wanted) fetched = svg;
    });
  });
  const svg = $derived(inlined ?? fetched);
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
