import { describe, it, expect } from "vitest";
import { showsDockedPanel } from "./dockedPanel.js";

const open = { hasRecipe: true, devPanel: true, detached: false, isTauri: true, spaceLook: false };

describe("showsDockedPanel", () => {
  it("shows the panel in Develop when it is open and docked", () => {
    expect(showsDockedPanel(open)).toBe(true);
  });

  it("never shows it during a Space quick look, even when it was left open", () => {
    expect(showsDockedPanel({ ...open, spaceLook: true })).toBe(false);
  });

  it("needs a recipe to edit, an open panel, and not to be detached", () => {
    expect(showsDockedPanel({ ...open, hasRecipe: false })).toBe(false);
    expect(showsDockedPanel({ ...open, devPanel: false })).toBe(false);
    expect(showsDockedPanel({ ...open, detached: true })).toBe(false);
    // Detached only counts inside the app; the web fixture has no second window.
    expect(showsDockedPanel({ ...open, detached: true, isTauri: false })).toBe(true);
  });
});
