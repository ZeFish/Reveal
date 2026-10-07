<script>
  import { Icon } from "@modules/core";

  /**
   * @typedef {Object} Props
   * @property {string} [label]
   * @property {boolean} [defaultCollapsed]
   * @property {import("svelte").Snippet} [children]
   */

  /** @type {Props} */
  let {
    label = "",
    defaultCollapsed = false,
    children,
  } = $props();

  const COLLAPSE_KEY = "reveal.engineRunner.collapsedGroups";

  function isInitiallyCollapsed() {
    if (!label) return false;
    try {
      const saved = JSON.parse(localStorage.getItem(COLLAPSE_KEY) || "[]");
      if (Array.isArray(saved) && saved.includes(label)) return true;
    } catch {}
    return defaultCollapsed;
  }

  let collapsed = $state(isInitiallyCollapsed());

  function toggle() {
    if (!label) return;
    collapsed = !collapsed;
    try {
      const saved = new Set(JSON.parse(localStorage.getItem(COLLAPSE_KEY) || "[]"));
      if (collapsed) saved.add(label);
      else saved.delete(label);
      localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...saved]));
    } catch {}
  }
</script>

<div class="collapsible-group">
  {#if label}
    <button
      type="button"
      class="group-header"
      onclick={toggle}
      aria-expanded={!collapsed}
    >
      <span class="chevron" class:collapsed>
        <Icon name="caret-down" size="var(--icon-sm)" />
      </span>
      <span class="group-title">{label}</span>
      <span class="group-line"></span>
    </button>
  {/if}

  {#if !label || !collapsed}
    <div class="group-controls">
      {@render children?.()}
    </div>
  {/if}
</div>

<style>
  .collapsible-group {
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .group-header {
    cursor: pointer;
    background: none;
    border: none;
    display: flex;
    align-items: center;
    gap: var(--space-d2);
    padding: var(--space-d2) 0;
    width: 100%;
    text-align: left;
    color: var(--color-muted);
    font-size: 0.65rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    transition: color var(--transition-fast);
  }
  .group-header:hover {
    color: var(--color-foreground);
  }
  .chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: transform var(--transition-fast);
  }
  .chevron.collapsed {
    transform: rotate(-90deg);
  }
  .group-title {
    font-family: var(--font-monospace);
    white-space: nowrap;
  }
  .group-line {
    flex: 1;
    height: 1px;
    background: var(--color-border);
    opacity: 0.6;
  }
  .group-controls {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding-inline-start: var(--space);
    padding-inline-end: var(--space-d2);
  }
</style>
