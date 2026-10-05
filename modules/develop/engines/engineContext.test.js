import { describe, it, expect } from "vitest";
import {
  getActiveTarget,
  toDisplay,
  fromDisplay,
  getNeutral,
  formatVal,
} from "./engineContext.js";

describe("engineContext", () => {
  it("resolves active target correctly for global and zone modes", () => {
    const recipe = { exposure_ev: 0.5 };
    expect(getActiveTarget(recipe, "global")).toBe(recipe);
    expect(getActiveTarget(recipe, undefined)).toBe(recipe);

    const shadowsTarget = getActiveTarget(recipe, "shadows");
    expect(shadowsTarget).toBe(recipe.zone_shadows);
    expect(recipe.zone_shadows).toBeDefined();
  });

  it("handles inverted controls like print_exposure_ev", () => {
    expect(toDisplay("print_exposure_ev", 1.5)).toBe(-1.5);
    expect(fromDisplay("print_exposure_ev", -1.5)).toBe(1.5);

    expect(toDisplay("exposure_ev", 1.5)).toBe(1.5);
    expect(fromDisplay("exposure_ev", 1.5)).toBe(1.5);
  });

  it("computes neutral values based on mode", () => {
    const defaults = { exposure_ev: 0, brightness: 5 };
    expect(getNeutral(defaults, "global", "brightness")).toBe(5);
    expect(getNeutral(defaults, "shadows", "brightness")).toBe(0);
    expect(getNeutral(defaults, "global", "unknown")).toBeUndefined();
  });

  it("formats values with correct units and signs", () => {
    expect(formatVal("temperature", 0)).toBe("5500 K");
    expect(formatVal("exposure_ev", 0.5)).toBe("+0.50");
    expect(formatVal("exposure_ev", -0.5)).toBe("-0.50");
    expect(formatVal("film_format_mm", 35.4)).toBe("35 mm");
    expect(formatVal("development_time_min", 0)).toBe("Auto");
    expect(formatVal("development_time_min", 6.5)).toBe("6.5 min");
  });
});
