<script>
  /**
   * @typedef {Object} SelectOption
   * @property {string} value
   * @property {string} label
   */

  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {string} value
   * @property {SelectOption[]} options
   * @property {boolean} [disabled]
   * @property {(value: string) => void} [onChange]
   */

  /** @type {Props} */
  let {
    label,
    value = "",
    options = [],
    disabled = false,
    onChange = () => {},
  } = $props();
</script>

<div class="frow" class:disabled>
  <span class="din frow-label" title={label}>{label}</span>
  <select
    class="panel-select"
    {value}
    {disabled}
    aria-label={label}
    onchange={(e) => onChange(e.currentTarget.value)}
  >
    {#each options as opt}
      <option value={opt.value}>{opt.label}</option>
    {/each}
  </select>
</div>

<style>
  .frow {
    display: flex;
    align-items: center;
    gap: var(--space-d3);
    min-height: 22px;
  }
  .frow.disabled {
    opacity: 0.38;
    pointer-events: none;
  }
  .frow-label {
    width: 6.2rem;
    flex-shrink: 0;
    font-size: 0.76rem;
    color: var(--color-foreground);
    text-align: left;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .panel-select {
    flex: 1;
    min-width: 0;
    height: 24px;
    font-size: 0.74rem;
    background: var(--color-surface);
    box-shadow: var(--shadow-border);
    border-radius: var(--radius);
    color: var(--color-foreground);
    padding: 0 var(--space-d3);
  }
</style>
