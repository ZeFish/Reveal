import { describe, it, expect, beforeEach } from "vitest";
import {
  library,
  beginOpen,
  appendFrames,
  refreshFrames,
  refreshLoadedFrames,
  clearFrames,
  leaveFolder,
  setRoots,
  dirLabel,
} from "./library.svelte.js";

/** @param {string[]} names */
const rows = (names) => names.map((n) => ({ path: `/nas/${n}.RAF`, name: n }));

describe("opening a folder", () => {
  beforeEach(() => {
    leaveFolder();
    setRoots([]);
  });

  it("moves to the folder and empties the grid at once", () => {
    beginOpen({ curDir: "/nas/A" });
    expect(library.curDir).toBe("/nas/A");
    expect(library.folder).toBe(null);
    expect(library.dir).toBe("/nas/A");
    expect(library.frames).toEqual([]);
    expect(library.loading).toBe(true);
  });

  it("commits its photos and settles", () => {
    const open = beginOpen({ curDir: "/nas/A" });
    expect(open.commit(rows(["a", "b"]))).toBe(true);
    open.finish();
    expect(library.frames.map((f) => f.name)).toEqual(["a", "b"]);
    expect(library.loading).toBe(false);
  });

  /**
   * The bug this pins. `openDir` guarded its commit with
   * `request !== applePhotosRequest` — a counter owned by the Apple Photos
   * path, captured on entry and never incremented by openDir itself. Between
   * two filesystem folders it compared N against N and always passed, so a
   * slow folder could land its photos under the name of the folder you had
   * already moved to. `openFolder` had no guard at all.
   *
   * Now the token IS the permission, and it expires the moment another open
   * starts.
   */
  it("refuses a load that has been overtaken", () => {
    const slow = beginOpen({ curDir: "/nas/big" });
    const quick = beginOpen({ curDir: "/nas/small" });

    expect(quick.commit(rows(["s1"]))).toBe(true);
    expect(slow.isCurrent).toBe(false);
    expect(slow.commit(rows(["b1", "b2", "b3"]))).toBe(false);

    expect(library.curDir).toBe("/nas/small");
    expect(library.frames.map((f) => f.name)).toEqual(["s1"]);
  });

  it("refuses a stale follow-up too, not just the first commit", () => {
    const slow = beginOpen({ curDir: "/nas/big" });
    slow.commit(rows(["b1"]));
    const quick = beginOpen({ curDir: "/nas/small" });
    quick.commit(rows(["s1"]));

    // The enriched list arriving late from the folder we left.
    expect(slow.replace(rows(["b1-enriched"]))).toBe(false);
    expect(library.frames.map((f) => f.name)).toEqual(["s1"]);
  });

  it("does not let an overtaken load clear the loading flag", () => {
    const slow = beginOpen({ curDir: "/nas/big" });
    beginOpen({ curDir: "/nas/small" });
    slow.finish();
    expect(library.loading).toBe(true); // the quick one is still loading
  });

  it("keeps the two kinds of folder apart", () => {
    beginOpen({ curDir: "/nas/indexed" });
    expect(library.dir).toBe("/nas/indexed");
    beginOpen({ folder: "/elsewhere/plain" });
    expect(library.curDir).toBe(null);
    expect(library.dir).toBe("/elsewhere/plain");
  });
});

describe("the photos of the open folder", () => {
  beforeEach(() => {
    leaveFolder();
  });

  it("appends a page without duplicating what is already there", () => {
    beginOpen({}).commit(rows(["a", "b"]));
    appendFrames(rows(["b", "c"]));
    expect(library.frames.map((f) => f.name)).toEqual(["a", "b", "c"]);
  });

  /**
   * The list is held raw, so a rating or a preview version written onto a
   * frame in place is invisible until the array itself is replaced.
   */
  it("re-publishes the list after a frame is changed in place", () => {
    beginOpen({}).commit(rows(["a"]));
    const before = library.frames;
    library.frames[0].rating = 5;
    expect(library.frames).toBe(before); // the mutation alone changes nothing
    refreshFrames();
    expect(library.frames).not.toBe(before);
    expect(library.frames[0].rating).toBe(5);
  });

  it("refreshes the loaded list without an open", () => {
    beginOpen({ curDir: "/nas/A" }).commit(rows(["a"]));
    refreshLoadedFrames(rows(["a", "b"]));
    expect(library.curDir).toBe("/nas/A"); // still the same folder
    expect(library.frames.map((f) => f.name)).toEqual(["a", "b"]);
  });

  it("clears the photos and leaves the folder behind", () => {
    beginOpen({ curDir: "/nas/A" }).commit(rows(["a"]));
    clearFrames();
    expect(library.frames).toEqual([]);
    expect(library.curDir).toBe("/nas/A");

    leaveFolder();
    expect(library.curDir).toBe(null);
    expect(library.dir).toBe(null);
  });
});

describe("labelling a directory", () => {
  it("names it relative to whichever root owns it", () => {
    setRoots(["/Volumes/home/Photos", "/mnt/ffp-production"]);
    expect(dirLabel("/mnt/ffp-production/Personelle/Kenya")).toBe("Personelle/Kenya");
    expect(dirLabel("/Volumes/home/Photos/2026")).toBe("2026");
  });

  it("falls back to the last segment for a directory no root owns", () => {
    setRoots(["/mnt/ffp-production"]);
    expect(dirLabel("/elsewhere/a/b")).toBe("b");
  });

  it("names a root by its own last segment", () => {
    setRoots(["/mnt/ffp-production"]);
    expect(dirLabel("/mnt/ffp-production")).toBe("ffp-production");
  });

  /** A sibling whose path merely starts with the same characters is not inside it. */
  it("does not treat a prefix as containment", () => {
    setRoots(["/mnt/ffp"]);
    expect(dirLabel("/mnt/ffp-production/x")).toBe("x");
  });
});
