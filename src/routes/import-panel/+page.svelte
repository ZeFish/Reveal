<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "$lib/components/Icon.svelte";

  type Outcome = "success" | "stopped" | "failure";
  // `archive` is stashed from the `import-started` event payload once import
  // begins (the destination folder, shown in the HUD) — not known at
  // discovery time, so it's optional and absent until then.
  type Card = { volume?: string; name: string; dcim: string; raw_count: number; archive?: string };
  type Progress = { done: number; total: number; current: string; path: string };

  let cards = $state<Card[]>([]);
  let importingCard = $state<Card | null>(null);
  let ejectableCard = $state<Card | null>(null);
  let progress = $state<Progress | null>(null);
  let outcome = $state<Outcome | null>(null);
  let summary = $state("");
  let ejecting = $state(false);

  // ETA bookkeeping — parity with the Swift HUD's `estimate()`. We record
  // the start time when the first progress tick arrives, and recompute the
  // remaining time from the average pace. `eta` is null until enough files
  // have copied for a stable average (and on the final tick).
  let startedAt = $state(0);
  let eta = $state<string | null>(null);
  let etaTimer = $state<ReturnType<typeof setInterval> | null>(null);

  // Auto-hide the finished HUD so it can never linger forever — matches the
  // Swift HUD's 30s `DispatchWorkItem` fallback. Cleared on any new import.
  let hideTimer = $state<ReturnType<typeof setTimeout> | null>(null);

  // `reveal://thumb?p=<path>` serves the camera's embedded JPEG (the same
  // protocol the contact sheet uses). Keyed on the path so each new file
  // swaps the image instead of showing the previous one's cached decode.
  function thumbUrl(path: string) {
    return `reveal://thumb?p=${encodeURIComponent(path)}`;
  }

  function clearTimers() {
    if (etaTimer) { clearInterval(etaTimer); etaTimer = null; }
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = null; }
  }

  function resetImportState() {
    progress = null;
    outcome = null;
    summary = "";
    eta = null;
    startedAt = 0;
    clearTimers();
  }

  /// Recompute the ETA string from the average pace so far. Mirrors the
  /// Swift `estimate()`: needs a couple of files and >1.5s elapsed, else
  /// null (the UI hides the field).
  function recomputeEta(done: number, total: number) {
    if (!startedAt || done === 0 || total <= done) { eta = null; return; }
    const elapsed = (Date.now() - startedAt) / 1000;
    if (elapsed < 1.5) { eta = null; return; }
    const remaining = (elapsed / done) * (total - done);
    eta = remaining < 60
      ? `~${Math.max(1, Math.round(remaining))} s`
      : `~${Math.round(remaining / 60)} min`;
  }

  // Unlisteners are registered async inside onMount; collected here so
  // onDestroy can tear them down. (Returning a cleanup from an async
  // onMount is a no-op in Svelte — the returned promise is ignored — so
  // we track them explicitly. This fixes a latent leak in the original.)
  const unlisteners: Array<() => void> = [];

  onMount(async () => {
    if (typeof document !== "undefined") {
      document.documentElement.classList.add("is-transparent-window");
      document.body.classList.add("is-transparent-window");
      document.documentElement.style.setProperty("background", "transparent", "important");
      document.documentElement.style.setProperty("background-color", "transparent", "important");
      document.documentElement.style.setProperty("background-image", "none", "important");
      document.body.style.setProperty("background", "transparent", "important");
      document.body.style.setProperty("background-color", "transparent", "important");
      document.body.style.setProperty("background-image", "none", "important");
    }

    const appWindow = getCurrentWindow();

    // Initial fetch
    cards = await invoke("find_cards");

    const unlistenProgress = await listen<Progress>("import-progress", (e) => {
      const { done, total, current, path } = e.payload;
      if (!startedAt) startedAt = Date.now();
      if (done === total) {
        // Final tick — the import-finished handler takes over from here.
        progress = null;
        eta = null;
        if (etaTimer) { clearInterval(etaTimer); etaTimer = null; }
      } else {
        progress = { done, total, current, path };
        recomputeEta(done, total);
        // Recompute once a second so the ETA ticks down between files.
        if (!etaTimer) {
          etaTimer = setInterval(() => {
            if (progress) recomputeEta(progress.done, progress.total);
          }, 1000);
        }
      }
    });

    const unlistenCards = await listen<Card[]>("cards-changed", (e) => {
      cards = e.payload;
    });

    const unlistenMounted = await listen("card-mounted", () => {
      // The backend auto-imports if enabled, but we just update UI.
    });

    const unlistenUnmounted = await listen<{ dcim: string }>("card-unmounted", (e) => {
      const dcim = e.payload.dcim;
      if (ejectableCard && ejectableCard.dcim === dcim) {
        ejectableCard = null;
        if (!importingCard) {
          appWindow.hide();
        }
      }
    });

    const unlistenStarted = await listen<{ dcim: string; archive?: string }>("import-started", (e) => {
      const dcim = e.payload.dcim;
      const card = cards.find(c => c.dcim === dcim) || { name: "Carte SD", dcim, raw_count: 0 };
      // Stash the destination so the HUD can show where photos are landing —
      // the whole point is the user never has to guess again.
      card.archive = e.payload.archive;
      importingCard = card;
      ejectableCard = null;
      resetImportState();
    });

    const unlistenFinished = await listen<{ copied?: number; skipped?: number; failed?: number; cancelled?: boolean }>("import-finished", async (e) => {
      const stats = e.payload;
      // "failure" is reserved for hard import-failed events (below). A
      // finished import with `failed > 0` files is still a success overall
      // — per-file failures are reported in the summary line.
      outcome = stats?.cancelled ? "stopped" : "success";
      const parts: string[] = [];
      if (stats?.copied) parts.push(`${stats.copied} imported`);
      if (stats?.skipped) parts.push(`${stats.skipped} skipped`);
      if (stats?.failed) parts.push(`${stats.failed} failed`);
      summary = parts.join(" · ") || "Done";
      if (importingCard && importingCard.volume) {
        ejectableCard = importingCard;
      }
      importingCard = null;
      cards = await invoke("find_cards");
      scheduleAutoHide();
    });

    const unlistenFailed = await listen<{ message?: string }>("import-failed", (e) => {
      outcome = "failure";
      summary = e.payload?.message || "Import failed";
      importingCard = null;
      scheduleAutoHide();
    });

    unlisteners.push(unlistenProgress, unlistenCards, unlistenMounted, unlistenUnmounted, unlistenStarted, unlistenFinished, unlistenFailed);
  });

  onDestroy(() => {
    for (const u of unlisteners) u();
    clearTimers();
  });

  function scheduleAutoHide() {
    if (hideTimer) clearTimeout(hideTimer);
    // 30s — matches the Swift HUD's fallback so a finished card never
    // lingers forever if the user walks away.
    hideTimer = setTimeout(() => {
      if (!importingCard) getCurrentWindow().hide();
    }, 30_000);
  }

  // Drag the HUD by its background — parity with the Swift HUD's
  // `panel.isMovableByWindowBackground = true`. Buttons stop propagation
  // naturally (pointer-events + their own handlers); this fires only on
  // background pointer-downs. `startDragging()` is async but we don't
  // await it — the OS takes over the gesture immediately.
  function startDrag(e: PointerEvent) {
    const target = e.target as HTMLElement;
    if (target.closest("button")) return;
    getCurrentWindow().startDragging();
  }

  async function cancelImport() {
    await invoke("cancel_import");
  }

  async function ejectCard(card: Card) {
    if (!card?.volume || ejecting) return;
    ejecting = true;
    try {
      await invoke("eject_card", { volume: card.volume });
      ejectableCard = null;
    } catch (e) {
      console.error(e);
    } finally {
      ejecting = false;
      if (!ejectableCard) {
        getCurrentWindow().hide();
      }
    }
  }

  let pct = $derived(progress && progress.total > 0 ? (progress.done / progress.total) * 100 : 0);
  let idleCard = $derived(!importingCard && !ejectableCard && !outcome && cards.length > 0 ? cards[0] : null);

  // Header icon/text by state — mirrors the Swift HUD's headerIcon/headerText.
  let headerIcon = $derived.by(() => {
    if (outcome === "success") return "check-circle";
    if (outcome === "stopped") return "stop-circle";
    if (outcome === "failure") return "warning";
    return "download-simple"; // importing
  });
  let headerText = $derived.by(() => {
    if (outcome === "success") return "Import complete";
    if (outcome === "stopped") return "Import stopped";
    if (outcome === "failure") return "Import failed";
    return "Importation";
  });

  $effect(() => {
    if (!importingCard && !ejectableCard && !idleCard && !outcome) {
      getCurrentWindow().hide();
    }
  });

  onDestroy(() => clearTimers());
</script>

<div class="panel-wrapper">
  <div class="panel" role="presentation" onpointerdown={startDrag}>
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
              <Icon name="download-simple" size="13px" />
              <span>Importation</span>
            </span>
            {#if progress && progress.total > 0}
              <span class="count">{progress.done}/{progress.total}</span>
            {:else}
              <span class="subtitle">{importingCard.name}</span>
            {/if}
          </div>
          {#if progress}
            <div class="progress-container">
              <div class="progress-bar">
                <div class="progress-fill" style="width: {pct}%"></div>
              </div>
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
              <button class="ghost-cancel-btn" onclick={cancelImport}>Stop</button>
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
            <Icon name={headerIcon} size="13px" class={outcome === "failure" ? "icon-warn" : outcome === "success" ? "icon-ok" : "icon-stopped"} />
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
            <button class="ghost-cancel-btn eject" onclick={() => ejectCard(ejectableCard!)} disabled={ejecting}>
              {ejecting ? "Ejecting..." : "Eject"}
            </button>
          {:else}
            <button class="ghost-cancel-btn" onclick={() => getCurrentWindow().hide()}>OK</button>
          {/if}
        </div>
      </div>
    {:else if idleCard}
      <div class="content">
        <div class="header">
          <span class="title">Card detected</span>
          <span class="subtitle">{idleCard.name}</span>
        </div>
        <p class="status-text">{idleCard.raw_count} RAWs disponibles</p>
        <div class="footer-row">
          <div></div>
          <button class="ghost-cancel-btn" onclick={() => { getCurrentWindow().hide(); invoke("show_main_window"); }}>
            Ouvrir Reveal
          </button>
        </div>
      </div>
    {:else}
      <div class="content empty">
        <p>En attente de carte...</p>
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
    font-family: var(--font-header, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif);
    color: #e5e5e5;
  }

  .panel-wrapper {
    width: 100%;
    height: 100%;
    padding: 6px;
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
    padding: 12px 14px;
    /* The whole background is a drag handle (parity with the Swift HUD's
       isMovableByWindowBackground). Buttons keep their own cursor via
       .action-btn. */
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

  /* HStack layout for the importing state: thumbnail beside text,
     matching the Swift HUD's `HStack { preview; VStack { … } }`. */
  .import-row {
    flex-direction: row;
    gap: 12px;
    align-items: stretch;
  }

  .import-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0; /* allow .file-name to ellipsis inside a flex child */
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
    margin-bottom: 12px;
  }

  .title {
    font-weight: 600;
    font-size: 13px;
    letter-spacing: 0.02em;
  }

  .subtitle {
    font-size: 11px;
    opacity: 0.6;
    font-family: var(--font-monospace, SFMono-Regular, Consolas, monospace);
  }

  .progress-container {
    margin-bottom: 12px;
  }

  .progress-bar {
    height: 6px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: var(--radius-sm);
    overflow: hidden;
    margin-bottom: 6px;
  }

  .progress-fill {
    height: 100%;
    background: var(--color-accent);
    border-radius: var(--radius-sm);
    transition: width 0.1s linear;
  }

  .progress-text {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 10px;
    opacity: 0.6;
    font-family: var(--font-text, sans-serif);
  }

  .file-name {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .status-text {
    font-size: 12px;
    opacity: 0.8;
    margin: 4px 0;
    flex: 1;
    font-family: var(--font-text, sans-serif);
  }

  /* The destination folder name under the progress bar — small, mono,
     ellipsized, with the full path on hover. Answers "where is this going?" */
  .dest-text {
    font-size: 10px;
    opacity: 0.55;
    margin: 4px 0 0;
    font-family: var(--font-monospace, SFMono-Regular, Consolas, monospace);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .success {
    color: #4CAF50;
  }

  .error {
    color: #E57373;
  }

  /* Title row with a leading icon (importing / finished outcome states). */
  .with-icon {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  /* The done/total count in the importing header — mono, dimmed, like the
     Swift HUD's `\(done)/\(total)`. */
  .count {
    font-size: 11px;
    opacity: 0.6;
    font-family: var(--font-monospace, SFMono-Regular, Consolas, monospace);
  }

  /* Outcome icon tints — success green, failure red, stopped neutral. */
  .panel :global(.icon-ok) {
    color: #4CAF50;
  }
  .panel :global(.icon-warn) {
    color: #E57373;
  }
  .panel :global(.icon-stopped) {
    color: #e5e5e5;
    opacity: 0.7;
  }

  /* ETA sits at the right of the progress text row, mono-digit so the
     width doesn't jitter as the number changes. The .file-name flexes to
     fill the space between the % and the ETA. */
  .eta {
    font-family: var(--font-monospace, SFMono-Regular, Consolas, monospace);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .footer-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: auto;
    gap: 12px;
    pointer-events: auto;
  }

  .ghost-cancel-btn {
    all: unset;
    cursor: pointer;
    font-family: var(--font-header, sans-serif);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.55);
    transition: color 0.15s var(--ease-standard);
    flex-shrink: 0;
    pointer-events: auto;
  }

  .ghost-cancel-btn:hover:not(:disabled) {
    color: #E57373;
  }

  .ghost-cancel-btn.eject:hover:not(:disabled) {
    color: #8bb4e6;
  }

  .ghost-cancel-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  
  .empty {
    justify-content: center;
    align-items: center;
    opacity: 0.4;
    font-size: 12px;
  }
</style>
