import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import {
  activity,
  notify,
  hold,
  dismiss,
  startActivity,
  updateActivity,
  setActive,
  releaseActive,
} from "./activity.svelte.js";

describe("the message line", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    dismiss();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("takes a message back after its own delay", () => {
    notify("saved", 3000);
    expect(activity.message).toBe("saved");
    vi.advanceTimersByTime(2999);
    expect(activity.message).toBe("saved");
    vi.advanceTimersByTime(1);
    expect(activity.message).toBe("");
  });

  /**
   * The bug this pins. Twenty-three call sites used to set the message and
   * schedule their own clear, with six different durations and no
   * cancellation between them. So a second message inherited the first one's
   * clock: "copied" at 6000ms followed by "renamed" at 3000ms left "renamed"
   * on screen for 3000ms of its own — and then the FIRST timer fired at 6000
   * and wiped whatever was up by then.
   */
  it("restarts the clock instead of inheriting the previous one", () => {
    notify("first", 6000);
    vi.advanceTimersByTime(5000);
    notify("second", 3000);

    // The first message's timer would have fired here, one second later.
    vi.advanceTimersByTime(1500);
    expect(activity.message).toBe("second");

    vi.advanceTimersByTime(1500);
    expect(activity.message).toBe("");
  });

  /**
   * Failures stay up. A bare assignment used to do this, and a bare
   * assignment left the previous message's timer running — so an error could
   * be wiped a second after it appeared.
   */
  it("holds a message with no timer, and cancels any pending one", () => {
    notify("saved", 3000);
    vi.advanceTimersByTime(1000);
    hold("Could not open photo: EPERM");

    vi.advanceTimersByTime(60_000);
    expect(activity.message).toBe("Could not open photo: EPERM");
  });

  it("dismisses on demand and cancels the timer with it", () => {
    notify("saved", 3000);
    dismiss();
    expect(activity.message).toBe("");
    notify("later", 3000);
    vi.advanceTimersByTime(2000);
    // The first notify's timer must not clear this one at its 3000 mark.
    expect(activity.message).toBe("later");
  });

  it("is read-only from outside", () => {
    notify("saved", 3000);
    expect(() => {
      // @ts-expect-error — svelte-check refuses this too; the runtime must
      // as well, or the guarantee is only a type-level one.
      activity.message = "smuggled in";
    }).toThrow();
    expect(activity.message).toBe("saved");
  });
});

describe("the job queue", () => {
  beforeEach(() => {
    for (const job of [...activity.queue]) updateActivity(job.id, { status: "completed" });
    setActive(null);
  });

  it("queues a job, makes it active, and reports it running", () => {
    const id = startActivity("export", "Export 12 photos", 12);
    expect(activity.activeId).toBe(id);
    expect(activity.anyRunning).toBe(true);
    const job = activity.queue.find((j) => j.id === id);
    expect(job).toMatchObject({ kind: "export", total: 12, done: 0, status: "running" });
  });

  it("patches one job and leaves its neighbours alone", () => {
    const a = startActivity("import", "Import", 10);
    const b = startActivity("export", "Export", 5);
    updateActivity(a, { done: 7, phase: "Copying" });

    expect(activity.queue.find((j) => j.id === a)).toMatchObject({ done: 7, phase: "Copying" });
    expect(activity.queue.find((j) => j.id === b)).toMatchObject({ done: 0, phase: "Queued" });
  });

  it("stops reporting work once every job has finished", () => {
    const id = startActivity("cull", "Cull", 3);
    updateActivity(id, { status: "completed" });
    expect(activity.anyRunning).toBe(false);
  });

  /**
   * The shape every `finally` block uses: a job gives the indicator back only
   * if it is still the one holding it. Releasing unconditionally would steal
   * the indicator from whatever started in the meantime.
   */
  it("releases the indicator only to its owner", () => {
    const mine = startActivity("publish", "Publish", 1);
    const theirs = startActivity("export", "Export", 1);
    expect(activity.activeId).toBe(theirs);

    releaseActive(mine); // finished late; the indicator is not mine any more
    expect(activity.activeId).toBe(theirs);

    releaseActive(theirs);
    expect(activity.activeId).toBe(null);
  });
});
