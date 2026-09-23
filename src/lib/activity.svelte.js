/**
 * What the app is telling you, and what it is busy doing.
 *
 * Two things that were loose variables in `+page.svelte`: the one-line
 * message at the bottom of the window, and the queue of long jobs behind the
 * indicator in the corner.
 *
 * The message had 46 call sites and, until this module, 23 of them set it
 * with their own `setTimeout` to clear it — six different durations, no
 * cancellation between them. A `notify()` doing it properly already existed
 * a few lines above; most callers simply did not use it. So two messages in
 * a row erased each other early: the first one's timer fired while the
 * second was still on screen.
 *
 * That is the same shape as the three bugs of 2026-09-22 — shared state,
 * several writers, no order anywhere — and the same answer applies. The
 * state below is module-private. What is exported is a READ-ONLY view plus
 * the handful of functions allowed to change it, and every one of them
 * cancels the pending timer first. An uncoordinated writer is not merely
 * discouraged here; there is no way to write one.
 *
 * Safe as a module singleton: `ssr = false` (src/routes/+layout.js), and the
 * dev/import/settings panels are separate Tauri webview windows with their
 * own JS context. `story-theme.svelte.js` relies on the same thing.
 */

/**
 * @typedef {Object} Activity
 * @property {string} id
 * @property {string} kind e.g. "import" | "export" | "cull" | "publish" | "move" | "develop"
 * @property {string} label
 * @property {string} current
 * @property {number} done
 * @property {number} total
 * @property {string} phase
 * @property {string} status "running" | "completed" | "failed" | "cancelled"
 * @property {string} timestamp
 */

const state = $state({
  /** @type {string} */ message: "",
  /** @type {Activity[]} */ queue: [],
  /** @type {string | null} */ activeId: null,
  queueOpen: false,
});

/** @type {ReturnType<typeof setTimeout> | null} */
let messageTimer = null;

function clearTimer() {
  if (messageTimer) clearTimeout(messageTimer);
  messageTimer = null;
}

/**
 * The read-only face of all of it. Reactive through the getters, so
 * components and effects track it exactly as they tracked the old `$state`
 * variables — they just cannot assign to it.
 */
export const activity = {
  get message() {
    return state.message;
  },
  get queue() {
    return state.queue;
  },
  get activeId() {
    return state.activeId;
  },
  get queueOpen() {
    return state.queueOpen;
  },
  /** Whether any job is still running — drives the ambient indicator. */
  get anyRunning() {
    return state.queue.some((job) => job.status === "running");
  },
};

/**
 * Say something, and take it back after `ms`.
 *
 * A second message replacing the first restarts the clock rather than
 * inheriting what was left of it.
 * @param {string} message
 * @param {number} [ms]
 */
export function notify(message, ms = 3000) {
  clearTimer();
  state.message = message;
  messageTimer = setTimeout(() => {
    state.message = "";
    messageTimer = null;
  }, ms);
}

/**
 * Say something and leave it up — for failures, which should stay until the
 * user has had a chance to read them or something else replaces them.
 *
 * Distinct from `notify` on purpose: a bare assignment used to do this, and
 * a bare assignment left the PREVIOUS message's timer running, so an error
 * could be wiped a second after it appeared.
 * @param {string} message
 */
export function hold(message) {
  clearTimer();
  state.message = message;
}

/** Take the message down now. */
export function dismiss() {
  clearTimer();
  state.message = "";
}

/**
 * Put a long job in the queue and make it the active one.
 *
 * Deliberately quiet: the corner indicator is the ambient signal and the
 * panel opens only when it is clicked (Francis: no window popping up every
 * time something starts).
 * @param {string} kind
 * @param {string} label
 * @param {number} total
 * @returns {string} the id, needed to update the same entry later
 */
export function startActivity(kind, label, total) {
  const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  state.queue = [
    ...state.queue,
    {
      id,
      kind,
      label,
      current: "Waiting",
      done: 0,
      total,
      phase: "Queued",
      status: "running",
      timestamp: new Date().toLocaleTimeString(),
    },
  ];
  state.activeId = id;
  return id;
}

/**
 * @param {string} id
 * @param {Partial<Activity>} patch
 */
export function updateActivity(id, patch) {
  state.queue = state.queue.map((job) => (job.id === id ? { ...job, ...patch } : job));
}

/**
 * Which job the indicator and progress bar are following.
 * @param {string | null} id
 */
export function setActive(id) {
  state.activeId = id;
}

/**
 * Stop following `id`, if it is the one being followed. The common shape in
 * a `finally`: whoever finishes gives the indicator back only if it still
 * belongs to them.
 * @param {string | null} id
 */
export function releaseActive(id) {
  if (id && state.activeId === id) state.activeId = null;
}

/** @param {boolean} [open] omit to toggle */
export function setQueueOpen(open) {
  state.queueOpen = open ?? !state.queueOpen;
}
