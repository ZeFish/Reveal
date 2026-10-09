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
   * @property {string} [hint] why the row is the way it is (shown on hover; useful when it is disabled)
   * @property {(value: string) => void} [onChange]
   */

  /** @type {Props} */
  let {
    label,
    value = "",
    options = [],
    disabled = false,
    hint = "",
    onChange = () => {},
  } = $props();
</script>

<div class="frow" class:disabled title={hint || undefined}>
  <span class="din frow-label">{label}</span>
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
  /* The field dims itself (the framework's disabled look); the label follows it. */
  .frow.disabled .frow-label {
    opacity: var(--opacity-disabled);
  }
  .frow.disabled {
    cursor: not-allowed;
  }
  .frow-label {
    width: 7.0rem;
    flex-shrink: 0;
    font-size: 0.76rem;
    color: var(--color-foreground);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-interface);
  }
  .panel-select {
    flex: 1;
    min-width: 0;
    font-size: var(--scale-d2);
    box-shadow: none;
    border-radius: var(--radius);
    color: var(--color-foreground);
    padding: 0 var(--space-d3);
  }
</style>
