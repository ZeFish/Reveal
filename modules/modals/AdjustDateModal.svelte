<script>
  import Dialog from "@stnd/ui/Dialog.svelte";

  /**
   * @typedef {Object} Props
   * @property {boolean} [open]
   * @property {any[]} [frames]
   * @property {() => void} [onClose]
   * @property {(updates: { path: string, capture_at: number }[]) => Promise<void> | void} [onAdjust]
   */

  /** @type {Props} */
  let {
    open = $bindable(false),
    frames = [],
    onClose = () => {},
    onAdjust = async () => {},
  } = $props();

  /** Reference frame (first selected or focused frame) */
  const refFrame = $derived(frames[0] ?? null);

  /** Parse reference capture timestamp in seconds */
  const originalTimestampSec = $derived(
    refFrame && refFrame.capture_at ? Number(refFrame.capture_at) : Math.floor(Date.now() / 1000)
  );

  /** Format Unix seconds into YYYY-MM-DDTHH:mm:ss for input type="datetime-local" */
  function timestampToLocalIso(/** @type {number} */ sec) {
    const d = new Date(sec * 1000);
    const pad = (/** @type {number} */ n) => String(n).padStart(2, "0");
    const y = d.getFullYear();
    const m = pad(d.getMonth() + 1);
    const day = pad(d.getDate());
    const h = pad(d.getHours());
    const min = pad(d.getMinutes());
    const s = pad(d.getSeconds());
    return `${y}-${m}-${day}T${h}:${min}:${s}`;
  }

  /** Format Unix seconds for human-readable display */
  function formatReadableDate(/** @type {number} */ sec) {
    const d = new Date(sec * 1000);
    return d.toLocaleString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  /** Current adjusted datetime string in local format */
  let adjustedIso = $state("");
  let submitting = $state(false);

  // Initialize input value when refFrame changes
  $effect(() => {
    if (open && originalTimestampSec) {
      adjustedIso = timestampToLocalIso(originalTimestampSec);
    }
  });

  /** Parse adjusted ISO string to Unix timestamp in seconds */
  const adjustedTimestampSec = $derived.by(() => {
    if (!adjustedIso) return originalTimestampSec;
    const d = new Date(adjustedIso);
    const time = d.getTime();
    return isNaN(time) ? originalTimestampSec : Math.floor(time / 1000);
  });

  /** Delta in seconds: adjusted - original */
  const deltaSec = $derived(adjustedTimestampSec - originalTimestampSec);

  /** Format delta for human readability */
  const deltaDescription = $derived.by(() => {
    if (deltaSec === 0) return "No change";
    const sign = deltaSec > 0 ? "+" : "−";
    const abs = Math.abs(deltaSec);
    const days = Math.floor(abs / 86400);
    const rem = abs % 86400;
    const hours = Math.floor(rem / 3600);
    const mins = Math.floor((rem % 3600) / 60);
    const secs = rem % 60;

    const parts = [];
    if (days >= 365) {
      const years = (days / 365).toFixed(1).replace(/\.0$/, "");
      parts.push(`${years} ${Number(years) === 1 ? "year" : "years"}`);
    } else if (days > 0) {
      parts.push(`${days}d`);
    }
    if (hours > 0) parts.push(`${hours}h`);
    if (mins > 0) parts.push(`${mins}m`);
    if (secs > 0 && days === 0) parts.push(`${secs}s`);

    return `${sign}${parts.join(" ")} (${sign}${abs.toLocaleString()}s)`;
  });

  function shiftYears(/** @type {number} */ years) {
    const d = new Date(adjustedTimestampSec * 1000);
    d.setFullYear(d.getFullYear() + years);
    adjustedIso = timestampToLocalIso(Math.floor(d.getTime() / 1000));
  }

  function shiftHours(/** @type {number} */ hours) {
    const d = new Date(adjustedTimestampSec * 1000);
    d.setHours(d.getHours() + hours);
    adjustedIso = timestampToLocalIso(Math.floor(d.getTime() / 1000));
  }

  function resetToOriginal() {
    adjustedIso = timestampToLocalIso(originalTimestampSec);
  }

  async function handleApply() {
    if (submitting) return;
    submitting = true;
    try {
      /** @type {{ path: string, capture_at: number }[]} */
      const updates = [];
      const offset = deltaSec;

      for (const f of frames) {
        if (!f?.path) continue;
        const baseTs = f.capture_at ? Number(f.capture_at) : originalTimestampSec;
        const newTs = baseTs + offset;
        updates.push({ path: f.path, capture_at: newTs });
      }

      await onAdjust(updates);
      onClose();
    } finally {
      submitting = false;
    }
  }

  function handleKeydown(/** @type {KeyboardEvent} */ event) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      handleApply();
    }
  }
</script>

{#if open}
  <Dialog bind:open label="Adjust date and time">
    <header class="modal-header">
      <h3>ADJUST DATE AND TIME</h3>
      <button class="close-btn" onclick={onClose} aria-label="Close dialog">✕</button>
    </header>

    <div class="modal-body" role="presentation" onkeydown={handleKeydown}>
      <div class="target-summary">
        {#if frames.length > 1}
          <span class="badge">{frames.length} photos selected</span>
          <span class="ref-name">Reference: {refFrame?.name || refFrame?.path?.split("/").pop()}</span>
        {:else}
          <span class="ref-name font-mono">{refFrame?.name || refFrame?.path?.split("/").pop()}</span>
        {/if}
      </div>

      <div class="field-row">
        <label for="orig-date" class="field-label">Original Date:</label>
        <div id="orig-date" class="field-value font-mono">
          {formatReadableDate(originalTimestampSec)}
        </div>
      </div>

      <div class="field-row">
        <label for="adjusted-date" class="field-label">Adjusted Date:</label>
        <div class="field-input-wrap">
          <input
            id="adjusted-date"
            type="datetime-local"
            step="1"
            bind:value={adjustedIso}
            class="datetime-input"
          />
        </div>
      </div>

      <div class="shift-presets">
        <span class="presets-label">Quick shift:</span>
        <div class="preset-buttons">
          <button type="button" class="preset-btn" onclick={() => shiftYears(-2)}>−2 years</button>
          <button type="button" class="preset-btn" onclick={() => shiftYears(-1)}>−1 year</button>
          <button type="button" class="preset-btn" onclick={() => shiftYears(1)}>+1 year</button>
          <button type="button" class="preset-btn" onclick={() => shiftHours(-1)}>−1 hr</button>
          <button type="button" class="preset-btn" onclick={() => shiftHours(1)}>+1 hr</button>
          <button type="button" class="preset-btn reset" onclick={resetToOriginal}>Reset</button>
        </div>
      </div>

      <div class="field-row delta-row">
        <span class="field-label">Time offset:</span>
        <div class="delta-value" class:changed={deltaSec !== 0}>
          {deltaDescription}
        </div>
      </div>

      {#if frames.length > 1}
        <p class="multi-hint">
          All {frames.length} selected photos will be shifted by the same time difference,
          preserving the exact time intervals between shots.
        </p>
      {/if}
    </div>

    <footer class="modal-footer">
      <button type="button" onclick={onClose} disabled={submitting}>Cancel</button>
      <button type="button" class="accent" onclick={handleApply} disabled={submitting}>
        {submitting ? "Adjusting…" : "Adjust"}
      </button>
    </footer>
  </Dialog>
{/if}

<style>
  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--space) calc(var(--space-d4) * 6);
    box-shadow: var(--shadow-border-bottom);
    background: var(--color-surface-sunken);
  }

  .modal-header h3 {
    margin: 0;
    font-size: 11px;
    letter-spacing: 0.08em;
    font-weight: 600;
  }

  .close-btn {
    cursor: pointer;
    opacity: 0.6;
    background: transparent;
    border: none;
    color: inherit;
    font-size: 13px;
  }

  .close-btn:hover {
    opacity: 1;
  }

  .modal-body {
    padding: calc(var(--space-d4) * 6);
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 4);
    min-width: 24rem;
  }

  .target-summary {
    display: flex;
    align-items: center;
    gap: calc(var(--space-d4) * 3);
    padding-bottom: calc(var(--space-d4) * 2);
    border-bottom: 1px solid var(--color-border);
  }

  .badge {
    background: var(--color-surface-raised);
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 500;
  }

  .ref-name {
    font-size: 12px;
    color: var(--color-muted);
  }

  .font-mono {
    font-family: var(--font-monospace);
  }

  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space);
  }

  .field-label {
    font-size: 12px;
    color: var(--color-muted);
    min-width: 6.5rem;
  }

  .field-value {
    font-size: 12px;
    color: var(--color-foreground);
  }

  .field-input-wrap {
    flex: 1;
    display: flex;
    justify-content: flex-end;
  }

  .datetime-input {
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    color: inherit;
    font-family: var(--font-monospace);
    font-size: 12px;
    padding: 4px 8px;
    outline: none;
    color-scheme: dark;
  }

  .datetime-input:focus {
    border-color: var(--color-primary);
  }

  .shift-presets {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-d4) * 2);
    background: var(--color-surface-sunken);
    padding: calc(var(--space-d4) * 3);
    border-radius: 6px;
  }

  .presets-label {
    font-size: 11px;
    color: var(--color-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .preset-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: calc(var(--space-d4) * 2);
  }

  .preset-btn {
    font-size: 11px;
    padding: 3px 8px;
    border-radius: 4px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    color: inherit;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .preset-btn:hover {
    background: var(--color-hover);
    border-color: var(--color-primary);
  }

  .preset-btn.reset {
    margin-left: auto;
    opacity: 0.8;
  }

  .delta-row {
    font-size: 12px;
  }

  .delta-value {
    font-family: var(--font-monospace);
    font-size: 12px;
    color: var(--color-muted);
  }

  .delta-value.changed {
    color: var(--color-primary);
    font-weight: 500;
  }

  .multi-hint {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
    color: var(--color-muted);
    border-top: 1px solid var(--color-border);
    padding-top: calc(var(--space-d4) * 2);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space);
    padding: var(--space) calc(var(--space-d4) * 6);
    box-shadow: var(--shadow-border-top);
    background: var(--color-surface-sunken);
  }

  .modal-footer button {
    padding: calc(var(--space-d4) * 2) calc(var(--space-d4) * 5);
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }

  .modal-footer button.accent {
    background: var(--color-primary);
    color: var(--color-primary-contrast);
    border: none;
    font-weight: 500;
  }
</style>
