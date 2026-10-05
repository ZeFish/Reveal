import { onMount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Photo } from "@modules/core";

/**
 * @typedef {"success" | "stopped" | "failure"} Outcome
 * @typedef {{ volume?: string; name: string; dcim: string; raw_count: number; archive?: string }} Card
 * @typedef {{ done: number; total: number; current: string; path: string }} Progress
 */

/**
 * Controller for the detached Import Panel (floating HUD).
 *
 * @param {Object} [deps]
 * @param {typeof invoke} [deps.invokeFn]
 * @param {typeof listen} [deps.listenFn]
 * @param {typeof getCurrentWindow} [deps.getWindowFn]
 */
export function createImportPanelMirror({
  invokeFn = invoke,
  listenFn = listen,
  getWindowFn = getCurrentWindow,
} = {}) {
  /** @type {Card[]} */
  let cards = $state([]);
  /** @type {Card | null} */
  let importingCard = $state(null);
  /** @type {Card | null} */
  let ejectableCard = $state(null);
  /** @type {Progress | null} */
  let progress = $state(null);
  /** @type {Outcome | null} */
  let outcome = $state(null);
  let summary = $state("");
  let ejecting = $state(false);

  let startedAt = $state(0);
  /** @type {string | null} */
  let eta = $state(null);
  /** @type {ReturnType<typeof setInterval> | null} */
  let etaTimer = $state(null);
  /** @type {ReturnType<typeof setTimeout> | null} */
  let hideTimer = $state(null);

  /** @param {string} path */
  function thumbUrl(path) {
    return Photo.thumb(path);
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

  /**
   * Recompute the ETA string from the average pace so far.
   * @param {number} done
   * @param {number} total
   */
  function recomputeEta(done, total) {
    if (!startedAt || done === 0 || total <= done) { eta = null; return; }
    const elapsed = (Date.now() - startedAt) / 1000;
    if (elapsed < 1.5) { eta = null; return; }
    const remaining = (elapsed / done) * (total - done);
    eta = remaining < 60
      ? `~${Math.max(1, Math.round(remaining))} s`
      : `~${Math.round(remaining / 60)} min`;
  }

  function scheduleAutoHide() {
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      if (!importingCard) {
        try { getWindowFn().hide(); } catch (_) {}
      }
    }, 30_000);
  }

  /** @param {PointerEvent} e */
  function startDrag(e) {
    const target = /** @type {HTMLElement} */ (e.target);
    if (target?.closest("button")) return;
    try {
      getWindowFn().startDragging();
    } catch (_) {}
  }

  async function cancelImport() {
    await invokeFn("cancel_import");
  }

  /** @param {Card} card */
  async function ejectCard(card) {
    if (!card?.volume || ejecting) return;
    ejecting = true;
    try {
      await invokeFn("eject_card", { volume: card.volume });
      ejectableCard = null;
    } catch (e) {
      console.error(e);
    } finally {
      ejecting = false;
      if (!ejectableCard) {
        try { getWindowFn().hide(); } catch (_) {}
      }
    }
  }

  function openReveal() {
    try {
      getWindowFn().hide();
      invokeFn("reveal_main_window");
    } catch (_) {}
  }

  function hideWindow() {
    try {
      getWindowFn().hide();
    } catch (_) {}
  }

  const pct = $derived.by(() => {
    const p = progress;
    if (!p || p.total <= 0) return 0;
    return (p.done / p.total) * 100;
  });
  const idleCard = $derived(!importingCard && !ejectableCard && !outcome && cards.length > 0 ? cards[0] : null);

  const headerIcon = $derived.by(() => {
    if (outcome === "success") return "check-circle";
    if (outcome === "stopped") return "stop-circle";
    if (outcome === "failure") return "warning";
    return "download-simple";
  });

  const headerText = $derived.by(() => {
    if (outcome === "success") return "Import complete";
    if (outcome === "stopped") return "Import stopped";
    if (outcome === "failure") return "Import failed";
    return "Importation";
  });

  /** @type {Array<() => void>} */
  const unlisteners = [];

  function destroy() {
    for (const u of unlisteners) u();
    clearTimers();
  }

  onMount(() => {
    (async () => {
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

      try {
        cards = await invokeFn("find_cards");
      } catch (_) {}

      try {
        const unlistenProgress = await listenFn("import-progress", (/** @type {any} */ e) => {
          const { done, total, current, path } = e.payload;
          if (!startedAt) startedAt = Date.now();
          if (done === total) {
            progress = null;
            eta = null;
            if (etaTimer) { clearInterval(etaTimer); etaTimer = null; }
          } else {
            progress = { done, total, current, path };
            recomputeEta(done, total);
            if (!etaTimer) {
              etaTimer = setInterval(() => {
                if (progress) recomputeEta(progress.done, progress.total);
              }, 1000);
            }
          }
        });

        const unlistenCards = await listenFn("cards-changed", (/** @type {any} */ e) => {
          cards = e.payload;
        });

        const unlistenMounted = await listenFn("card-mounted", () => {});

        const unlistenUnmounted = await listenFn("card-unmounted", (/** @type {any} */ e) => {
          const dcim = e.payload.dcim;
          if (ejectableCard && ejectableCard.dcim === dcim) {
            ejectableCard = null;
            if (!importingCard) {
              hideWindow();
            }
          }
        });

        const unlistenStarted = await listenFn("import-started", (/** @type {any} */ e) => {
          const dcim = e.payload.dcim;
          const matched = cards.find(c => c.dcim === dcim);
          /** @type {Card} */
          const card = matched ? { ...matched } : { name: "Carte SD", dcim, raw_count: 0 };
          card.archive = e.payload.archive;
          importingCard = card;
          ejectableCard = null;
          resetImportState();
        });

        const unlistenFinished = await listenFn("import-finished", async (/** @type {any} */ e) => {
          const stats = e.payload;
          outcome = stats?.cancelled ? "stopped" : "success";
          const parts = [];
          if (stats?.copied) parts.push(`${stats.copied} imported`);
          if (stats?.skipped) parts.push(`${stats.skipped} skipped`);
          if (stats?.failed) parts.push(`${stats.failed} failed`);
          summary = parts.join(" · ") || "Done";
          if (importingCard && importingCard.volume) {
            ejectableCard = importingCard;
          }
          importingCard = null;
          try {
            cards = await invokeFn("find_cards");
          } catch (_) {}
          scheduleAutoHide();
        });

        const unlistenFailed = await listenFn("import-failed", (/** @type {any} */ e) => {
          outcome = "failure";
          summary = e.payload?.message || "Import failed";
          importingCard = null;
          scheduleAutoHide();
        });

        unlisteners.push(
          unlistenProgress,
          unlistenCards,
          unlistenMounted,
          unlistenUnmounted,
          unlistenStarted,
          unlistenFinished,
          unlistenFailed
        );
      } catch (_) {}
    })();

    return () => {
      destroy();
    };
  });

  $effect(() => {
    if (!importingCard && !ejectableCard && !idleCard && !outcome) {
      hideWindow();
    }
  });

  return {
    get panelProps() {
      return {
        cards,
        importingCard,
        ejectableCard,
        idleCard,
        progress,
        outcome,
        summary,
        ejecting,
        pct,
        eta,
        headerIcon,
        headerText,
        thumbUrl,
        onStartDrag: startDrag,
        onCancelImport: cancelImport,
        onEjectCard: ejectCard,
        onOpenReveal: openReveal,
        onHide: hideWindow,
      };
    },
    // Expose internals for direct control or test assertions
    get cards() { return cards; },
    get importingCard() { return importingCard; },
    get ejectableCard() { return ejectableCard; },
    get progress() { return progress; },
    get outcome() { return outcome; },
    get summary() { return summary; },
    get ejecting() { return ejecting; },
    cancelImport,
    ejectCard,
    thumbUrl,
    resetImportState,
    scheduleAutoHide,
    clearTimers,
  };
}
