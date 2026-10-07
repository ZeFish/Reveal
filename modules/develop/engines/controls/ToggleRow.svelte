<script>
  /**
   * @typedef {Object} Props
   * @property {string} label
   * @property {boolean} checked
   * @property {boolean} [disabled]
   * @property {(checked: boolean) => void} [onChange]
   */

  /** @type {Props} */
  let {
    label,
    checked = false,
    disabled = false,
    onChange = () => {},
  } = $props();
</script>

<div class="frow" class:disabled>
  <span class="din frow-label" title={label}>{label}</span>
  <span class="spacer"></span>
  <span class="val toggle-wrap">
    <input
      type="checkbox"
      role="switch"
      aria-label={label}
      {checked}
      {disabled}
      onchange={(e) => onChange(e.currentTarget.checked)}
    />
  </span>
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
  .spacer {
    flex: 1;
  }
  .toggle-wrap {
    display: inline-flex;
    justify-content: flex-end;
  }
</style>
