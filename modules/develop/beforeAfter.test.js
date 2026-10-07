import { describe, expect, it } from "vitest";
import { createBeforeAfter, HOLD_MS } from "./beforeAfter.js";

/** A clock that only moves when told to. */
function setup() {
  /** @type {boolean[]} */
  const told = [];
  /** @type {{ fn: () => void, at: number, live: boolean }[]} */
  const timers = [];
  let now = 0;
  const ba = createBeforeAfter({
    onChange: (v) => told.push(v),
    schedule: (fn, ms) => {
      const t = { fn, at: now + ms, live: true };
      timers.push(t);
      return t;
    },
    cancel: (t) => {
      t.live = false;
    },
  });
  const advance = (ms) => {
    now += ms;
    for (const t of timers) if (t.live && t.at <= now) { t.live = false; t.fn(); }
  };
  return { ba, told, advance };
}

describe("before / after", () => {
  it("a tap shows the photo as shot and stays there; the next tap brings the edit back", () => {
    const { ba, told, advance } = setup();
    ba.press(); advance(HOLD_MS - 50); ba.release();
    expect(told).toEqual([true]);
    ba.press(); advance(HOLD_MS - 50); ba.release();
    expect(told).toEqual([true, false]);
  });

  it("a hold shows the photo as shot while the key is down and lets go on release", () => {
    const { ba, told, advance } = setup();
    ba.press(); advance(HOLD_MS + 10);
    expect(told).toEqual([true]);
    ba.release();
    expect(told).toEqual([true, false]);
  });

  it("a hold on top of a tap comes back to the before it was left on", () => {
    const { ba, told, advance } = setup();
    ba.press(); ba.release(); // tap: before is on
    ba.press(); advance(HOLD_MS + 10); ba.release(); // hold, then let go
    expect(told[told.length - 1]).toBe(true);
  });

  it("a key that repeats while held is one press", () => {
    const { ba, told, advance } = setup();
    ba.press(); ba.press(); ba.press(); advance(HOLD_MS + 10);
    ba.release();
    expect(told).toEqual([true, false]);
  });

  it("a release with no press does nothing", () => {
    const { ba, told } = setup();
    ba.release();
    expect(told).toEqual([]);
  });

  it("reset goes back to the developed picture and forgets a half-made press", () => {
    const { ba, told, advance } = setup();
    ba.press(); ba.release(); // before on
    ba.reset();
    expect(told).toEqual([true, false]);
    ba.press(); ba.reset(); advance(HOLD_MS + 10); // the timer must not fire
    expect(told).toEqual([true, false]);
  });
});
