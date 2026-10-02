/**
 * Updates: find out a newer Reveal exists, fetch it, swap it in, relaunch.
 *
 * The mechanics are Tauri's updater plugin (signature-checked against the
 * public key in tauri.conf.json, feed at ZeFish/Reveal releases). The plugin
 * ships no interface, so everything the person sees is ours: a quiet card
 * when something is ready, release notes taken from the CHANGELOG section the
 * release script wrote into latest.json, and a "What's new" once after the
 * relaunch.
 *
 * State is a module singleton on the same grounds as `activity.svelte.js`:
 * `ssr = false`, and every Tauri window has its own JS context. The main
 * window and the Settings window each run their own copy, so the only thing
 * they share is localStorage — which is exactly what carries "what's new"
 * across the relaunch.
 */

import { isTauri } from "./api.js";

const WHATS_NEW_KEY = "reveal.whatsNew";

/** @typedef {"idle" | "checking" | "uptodate" | "available" | "downloading" | "ready" | "error"} Status */

const state = $state({
  /** @type {Status} */ status: "idle",
  version: "",
  notes: "",
  /** 0–1, or -1 while the size is unknown. */
  progress: 0,
  error: "",
});

/** The plugin's `Update` handle for the version in `state`, once found. */
/** @type {import("@tauri-apps/plugin-updater").Update | null} */
let pending = null;

export const updater = {
  get status() {
    return state.status;
  },
  get version() {
    return state.version;
  },
  get notes() {
    return state.notes;
  },
  get progress() {
    return state.progress;
  },
  get error() {
    return state.error;
  },
  /** An update is known and not yet installed — what the card keys on. */
  get hasUpdate() {
    return state.status === "available" || state.status === "downloading" || state.status === "ready";
  },
};

/**
 * Turn the changelog markdown carried by an update into something to render.
 * Understands exactly what `release.mjs` writes: `### Heading` then `- item`.
 * @param {string | undefined} notes
 * @returns {{ heading: string, items: string[] }[]}
 */
export function parseNotes(notes) {
  /** @type {{ heading: string, items: string[] }[]} */
  const groups = [];
  for (const raw of String(notes ?? "").split("\n")) {
    const line = raw.trim();
    const heading = line.match(/^#{2,4}\s+(.*)$/);
    if (heading) {
      // The `## 0.47.0 — date` title repeats the version already on screen.
      if (!/^\d+\.\d+\.\d+/.test(heading[1])) groups.push({ heading: heading[1], items: [] });
      continue;
    }
    const item = line.match(/^[-*]\s+(.*)$/);
    if (item) {
      if (!groups.length) groups.push({ heading: "", items: [] });
      groups[groups.length - 1].items.push(item[1]);
    }
  }
  return groups.filter((g) => g.items.length);
}

/**
 * Ask the feed whether a newer version exists.
 *
 * `quiet` is for the check at launch: nobody asked, so "up to date" and
 * "couldn't reach GitHub" both leave no trace. A check the person asked for
 * (Settings → About) reports either.
 * @param {{ quiet?: boolean }} [options]
 */
export async function checkForUpdate({ quiet = false } = {}) {
  if (!isTauri || state.status === "checking" || state.status === "downloading") return;
  if (updater.hasUpdate) return;
  state.status = "checking";
  state.error = "";
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) {
      state.status = quiet ? "idle" : "uptodate";
      return;
    }
    pending = update;
    state.version = update.version;
    state.notes = update.body ?? "";
    state.status = "available";
  } catch (e) {
    state.status = quiet ? "idle" : "error";
    state.error = e instanceof Error ? e.message : String(e);
  }
}

/** Download the pending update, install it, and relaunch into it. */
export async function installUpdate() {
  if (!pending || state.status !== "available") return;
  state.status = "downloading";
  state.progress = 0;
  state.error = "";
  try {
    let total = 0;
    let received = 0;
    await pending.downloadAndInstall((/** @type {any} */ event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? 0;
        state.progress = total ? 0 : -1;
      } else if (event.event === "Progress") {
        received += event.data.chunkLength;
        if (total) state.progress = Math.min(received / total, 1);
      } else if (event.event === "Finished") {
        state.progress = 1;
      }
    });
    state.status = "ready";
    // Written BEFORE the relaunch, read by the next launch.
    writeWhatsNew({ version: state.version, notes: state.notes });
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch (e) {
    state.status = "error";
    state.error = e instanceof Error ? e.message : String(e);
  }
}

/** "Later": hide the card until the next launch. */
export function dismissUpdate() {
  if (state.status === "available") state.status = "idle";
}

/**
 * What changed in the version that was just installed — once. Returns `null`
 * on every launch that did not follow an update, and after the first read.
 * @returns {{ version: string, notes: string } | null}
 */
export function takeWhatsNew() {
  const stored = readWhatsNew();
  if (!stored) return null;
  try {
    localStorage.removeItem(WHATS_NEW_KEY);
  } catch (_) {}
  return stored;
}

/** @param {{ version: string, notes: string }} value */
function writeWhatsNew(value) {
  try {
    localStorage.setItem(WHATS_NEW_KEY, JSON.stringify(value));
  } catch (_) {
    // Losing the "what's new" screen is not worth failing an install over.
  }
}

function readWhatsNew() {
  try {
    const parsed = JSON.parse(localStorage.getItem(WHATS_NEW_KEY) ?? "null");
    if (parsed && typeof parsed.version === "string" && typeof parsed.notes === "string") return parsed;
  } catch (_) {}
  return null;
}
