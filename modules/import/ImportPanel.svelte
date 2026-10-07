<script>
  import { Icon } from "@modules/core";

  /** @typedef {import('./importPanelMirror.svelte.js').Card} Card */
  /** @typedef {import('./importPanelMirror.svelte.js').Progress} Progress */
  /** @typedef {import('./importPanelMirror.svelte.js').Outcome} Outcome */

  /**
   * @typedef {Object} Props
   * @property {Card[]} [cards]
   * @property {Card | null} [importingCard]
   * @property {Card | null} [ejectableCard]
   * @property {Card | null} [idleCard]
   * @property {Progress | null} [progress]
   * @property {Outcome | null} [outcome]
   * @property {string} [summary]
   * @property {boolean} [ejecting]
   * @property {number} [pct]
   * @property {string | null} [eta]
   * @property {string} [headerIcon]
   * @property {string} [headerText]
   * @property {(path: string) => string} [thumbUrl]
   * @property {(e: PointerEvent) => void} [onStartDrag]
   * @property {() => void} [onCancelImport]
   * @property {(card: Card) => void} [onEjectCard]
   * @property {() => void} [onOpenReveal]
   * @property {() => void} [onHide]
   */

  /** @type {Props} */
  let {
    cards = [],
    importingCard = null,
    ejectableCard = null,
    idleCard = null,
    progress = null,
    outcome = null,
    summary = "",
    ejecting = false,
    pct = 0,
    eta = null,
    headerIcon = "download-simple",
    headerText = "Importing",
    thumbUrl = (p) => `reveal://thumb?p=${encodeURIComponent(p)}`,
    onStartDrag = () => {},
    onCancelImport = () => {},
    onEjectCard = () => {},
    onOpenReveal = () => {},
    onHide = () => {},
  } = $props();
</script>

<div class="panel-wrapper">
  <div class="panel" role="presentation" onpointerdown={onStartDrag}>
    {#if importingCard}
      <div class="content import-row">
        {#if progress?.path}
          <!-- Live preview of the frame being copied: the camera's embedded
               JPEG, pulled from the RAW on the card. Keyed on the path so
               each new file swaps the <img> rather than reusing the prior
               decode (browsers would otherwise cache by URL). -->
          <div class="thumb-wrap">
            {#key progress.path}
              <img class="thumb" src={thumbUrl(progress.path)} alt="" />
            {/key}
          </div>
        {/if}
        <div class="import-text">
          <div class="header">
            <span class="title with-icon">
              <Icon name="download-simple" size="var(--icon-lg)" />
              <span>Importing</span>
            </span>
            {#if progress && progress.total > 0}
              <span class="count">{progress.done}/{progress.total}</span>
            {:else}
              <span class="subtitle">{importingCard.name}</span>
            {/if}
          </div>
          {#if progress}
            <div class="progress-container">
              <progress value={pct} max="100"></progress>
              <div class="progress-text">
                <span>{Math.round(pct)}%</span>
                <span class="file-name" title={progress.current}>{progress.current || "..."}</span>
                {#if eta}<span class="eta">{eta}</span>{/if}
              </div>
            </div>
            <div class="footer-row">
              {#if importingCard?.archive}
                <!-- Where the photos are landing — the destination folder name
                     with the full path as a tooltip, so the HUD answers
                     "where is this going?" at a glance. -->
                <p class="dest-text" title={importingCard.archive}>
                  → {importingCard.archive.split("/").filter(Boolean).pop() || importingCard.archive}
                </p>
              {:else}
                <div></div>
              {/if}
              <button class="ghost-cancel-btn" onclick={onCancelImport}>Stop</button>
            </div>
          {:else}
            <p class="status-text">Preparing...</p>
          {/if}
        </div>
      </div>
    {:else if outcome}
      <!-- Finished state — outcome icon + summary, plus an eject action when
           a real card is still mounted (the ingest loop's last step, offered
           in the HUD rather than forced). Auto-hides after 30s. -->
      <div class="content">
        <div class="header">
          <span class="title with-icon">
            <Icon name={headerIcon} size="var(--icon-lg)" class={outcome === "failure" ? "icon-warn" : outcome === "success" ? "icon-ok" : "icon-stopped"} />
            <span>{headerText}</span>
          </span>
          {#if ejectableCard}
            <span class="subtitle">{ejectableCard.name}</span>
          {/if}
        </div>
        <p class="status-text {outcome === 'success' ? 'success' : outcome === 'failure' ? 'error' : ''}">{summary}</p>
        <div class="footer-row">
          <div></div>
          {#if ejectableCard}
            <button class="ghost-cancel-btn eject" onclick={() => onEjectCard(ejectableCard)} disabled={ejecting}>
              {ejecting ? "Ejecting..." : "Eject"}
            </button>
          {:else}
            <button class="ghost-cancel-btn" onclick={onHide}>OK</button>
          {/if}
        </div>
      </div>
    {:else if idleCard}
      <div class="content">
        <div class="header">
          <span class="title">Card detected</span>
          <span class="subtitle">{idleCard.name}</span>
        </div>
        <p class="status-text">{idleCard.raw_count} RAW{idleCard.raw_count === 1 ? "" : "s"} available</p>
        <div class="footer-row">
          <div></div>
          <button class="ghost-cancel-btn" onclick={onOpenReveal}>
            Open Reveal
          </button>
        </div>
      </div>
    {:else}
      <div class="content empty">
        <p>Waiting for a card…</p>
      </div>
    {/if}
  </div>
</div>

<style>
  :global(html),
  :global(html[data-theme]),
  :global(body) {
    margin: 0 !important;
    padding: 0 !important;
    width: 100% !important;
    height: 100% !important;
    background: transparent !important;
    background-color: transparent !important;
    background-image: none !important;
    overflow: hidden !important;
    font-family: var(--font-interface, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif);
    color: #e5e5e5;
  }

  .panel-wrapper {
    width: 100%;
    height: 100%;
    padding: var(--space-d3);
    box-sizing: border-box;
    display: flex;
    background: transparent;
  }

  .panel {
    flex: 1;
    width: 100%;
    height: 100%;
    background: rgba(30, 30, 30, 0.82);
    backdrop-filter: blur(24px) saturate(150%);
    -webkit-backdrop-filter: blur(24px) saturate(150%);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-raised), var(--shadow-lift);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    padding: calc(var(--space-d4) * 3) var(--space);
    cursor: default;
  }

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    pointer-events: none;
  }

  .content > * {
    pointer-events: auto;
  }

  /* HStack layout for the importing state: thumbnail beside text */
  .import-row {
    flex-direction: row;
    gap: calc(var(--space-d4) * 3);
    align-items: stretch;
  }

  .import-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .thumb-wrap {
    width: 64px;
    height: 64px;
    flex-shrink: 0;
    border-radius: var(--radius);
    overflow: hidden;
    background: rgba(255, 255, 255, 0.06);
    align-self: center;
  }

  .thumb {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: calc(var(--space-d4) * 3);
  }

  .subtitle {
    opacity: 0.6;
  }

  .progress-container {
    display: flex;
    flex-direction: column;
    gap: var(--space-d3);
    margin-bottom: var(--space-d3);
  }

  .progress-text {
    display: flex;
    align-items: baseline;
    gap: var(--space-d2);
    opacity: 0.6;
  }

  .file-name {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-text {
    opacity: 0.8;
    margin: var(--space-d4) 0;
    flex: 1;
  }

  .dest-text {
    opacity: 0.55;
    margin: var(--space-d4) 0 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .success {
    color: var(--color-green);
  }

  .error {
    color: var(--color-red);
  }

  .with-icon {
    display: inline-flex;
    align-items: center;
    gap: var(--space-d3);
  }

  .count {
    opacity: 0.6;
  }

  .panel :global(.icon-ok) {
    color: var(--color-green);
  }
  .panel :global(.icon-warn) {
    color: var(--color-red);
  }
  .panel :global(.icon-stopped) {
    color: #e5e5e5;
    opacity: 0.7;
  }

  .eta {
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .footer-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: auto;
    gap: calc(var(--space-d4) * 3);
    pointer-events: auto;
  }

  .ghost-cancel-btn {
    cursor: pointer;
    flex-shrink: 0;
    pointer-events: auto;
    /* The same height as the buttons in the panels. */
    min-height: var(--control-h);
    padding-block: 0;
    display: inline-flex;
    align-items: center;
  }

  .ghost-cancel-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .empty {
    justify-content: center;
    align-items: center;
    opacity: 0.4;
  }
</style>
