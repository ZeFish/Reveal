<script module>
  // Every Phosphor icon in @stnd/icon, each as its own lazily-loaded chunk.
  //
  // modules/core/icons.js inlines the icons the app shows on first paint, so those
  // render with no wait. Anything not inlined loads from the package itself.
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
  import { icons } from "../icons.js";

  let { name, size = "13px", class: className = "" } = $props();

  const inlined = $derived(icons[/** @type {keyof typeof icons} */ (name)]);
  let fetched = $state("");
  $effect(() => {
    if (inlined) return;
    let cancelled = false;
    load(name).then((svg) => {
      if (!cancelled) fetched = svg;
    });
    return () => {
      cancelled = true;
    };
  });
  const rawSvg = $derived(inlined || fetched);

  // Phosphor SVGs come with hardcoded width="256" height="256". Replace those
  // with the requested size so the icon sizes cleanly without external CSS.
  const svg = $derived(
    rawSvg
      ? rawSvg
          .replace(/width="\d+"/, `width="${size}"`)
          .replace(/height="\d+"/, `height="${size}"`)
      : ""
  );
</script>

{#if svg}
  <!-- Inlined SVGs from our own package — safe to render raw -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="reveal-icon {className}"
    style="--icon-size: {size}; display: inline-flex; align-items: center; justify-content: center; width: {size}; height: {size}; line-height: 1; flex-shrink: 0;"
    aria-hidden="true"
  >{@html svg}</span>
{/if}
