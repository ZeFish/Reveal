import { describe, expect, it } from "vitest";
import { parseNotes } from "./updater.svelte.js";

describe("modules/updates/parseNotes", () => {
  it("reads the sections release.mjs writes, and drops the version title", () => {
    const notes = [
      "## 0.47.0 — 2026-10-09",
      "",
      "### Added",
      "",
      "- Update card",
      "- What's new after relaunch",
      "",
      "### Fixed",
      "",
      "- A crash on launch",
    ].join("\n");
    expect(parseNotes(notes)).toEqual([
      { heading: "Added", items: ["Update card", "What's new after relaunch"] },
      { heading: "Fixed", items: ["A crash on launch"] },
    ]);
  });

  it("keeps bullets that come before any heading", () => {
    expect(parseNotes("- one\n- two")).toEqual([{ heading: "", items: ["one", "two"] }]);
  });

  it("drops sections with nothing in them, and survives missing notes", () => {
    expect(parseNotes("### Added\n\n### Fixed\n- x")).toEqual([{ heading: "Fixed", items: ["x"] }]);
    expect(parseNotes(undefined)).toEqual([]);
    expect(parseNotes("_Internal changes only._")).toEqual([]);
  });
});
