import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { session, openFolderSession } from "./session.js";

/**
 * The module reaches for `localStorage` lazily on every read and write, so a
 * plain in-memory stand-in installed here is enough — no DOM, no jsdom.
 */
function installStorage() {
  /** @type {Map<string, string>} */
  const map = new Map();
  globalThis.localStorage = /** @type {any} */ ({
    getItem: (/** @type {string} */ k) => (map.has(k) ? map.get(k) : null),
    setItem: (/** @type {string} */ k, /** @type {unknown} */ v) => map.set(k, String(v)),
    removeItem: (/** @type {string} */ k) => map.delete(k),
    clear: () => map.clear(),
    get length() {
      return map.size;
    },
    key: (/** @type {number} */ i) => [...map.keys()][i] ?? null,
  });
  return map;
}

describe("session memory", () => {
  /** @type {Map<string, string>} */
  let store;
  beforeEach(() => {
    store = installStorage();
  });
  afterEach(() => {
    // @ts-expect-error — putting the environment back as it was found
    delete globalThis.localStorage;
  });

  /**
   * The bug this pins. The grid always reopened at the top however far you
   * had scrolled, because an `$effect` persisted the scroll at boot — before
   * anything had been read — and wrote the initial 0 over the real value,
   * which the loader then read back as 0. Nothing in the code stated an
   * order between saving and loading.
   *
   * There is now nothing to write through until the read has happened: the
   * writers come back FROM `openFolderSession`, so a folder nobody has
   * opened has no writer at all. The test below is the shape of that: the
   * only path to `saveScroll` runs through the read.
   */
  it("hands back the writers only with the values", () => {
    const dir = "/nas/2026/A";
    const first = openFolderSession(dir);
    expect(first.scroll).toBe(0);
    expect(first.mode).toBe(null);

    first.saveScroll(1376);
    first.saveMode("dev");

    const reopened = openFolderSession(dir);
    expect(reopened.scroll).toBe(1376);
    expect(reopened.mode).toBe("dev");
  });

  it("binds each writer to its own folder", () => {
    const a = openFolderSession("/nas/A");
    const b = openFolderSession("/nas/B");
    a.saveScroll(500);

    expect(openFolderSession("/nas/A").scroll).toBe(500);
    expect(openFolderSession("/nas/B").scroll).toBe(0);
    expect(b.scroll).toBe(0);
  });

  it("validates a stored mode rather than casting it", () => {
    // "story" was a real mode once; a vault carried over from that era must
    // not mount a surface that no longer exists.
    store.set("reveal.mode./nas/A", "story");
    expect(openFolderSession("/nas/A").mode).toBe(null);
  });

  it("refuses a scroll offset that is not a usable number", () => {
    for (const junk of ["", "NaN", "-40", "later", "Infinity"]) {
      store.set("reveal.scroll./nas/A", junk);
      expect(openFolderSession("/nas/A").scroll).toBe(0);
    }
  });

  it("validates the global mode too", () => {
    store.set("reveal.currentMode", "cull");
    expect(session.lastMode()).toBe("cull");
    store.set("reveal.currentMode", "nonsense");
    expect(session.lastMode()).toBe(null);
  });

  it("degrades to the default when a JSON value is corrupt", () => {
    store.set("reveal.grid", "{not json");
    expect(session.gridPrefs()).toBe(null);
    store.set("reveal.grid", '"a string, not an object"');
    expect(session.gridPrefs()).toBe(null);
    session.setGridPrefs({ cols: 4 });
    expect(session.gridPrefs()).toEqual({ cols: 4 });
  });

  /**
   * Storage is absent in a plain browser tab and throws outright in some
   * private-mode configurations. Neither may take an edit down with it.
   */
  it("survives storage being absent or hostile", () => {
    // @ts-expect-error
    delete globalThis.localStorage;
    expect(() => session.setLastPhoto("/nas/A/x.RAF")).not.toThrow();
    expect(session.lastPhoto()).toBe(null);
    expect(openFolderSession("/nas/A").scroll).toBe(0);

    globalThis.localStorage = /** @type {any} */ ({
      getItem() {
        throw new Error("blocked");
      },
      setItem() {
        throw new Error("blocked");
      },
    });
    expect(() => session.setLastPhoto("/nas/A/x.RAF")).not.toThrow();
    expect(session.lastPhoto()).toBe(null);
  });
});
