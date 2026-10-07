import { describe, it, expect } from "vitest";
import {
  getActiveTarget,
  toDisplay,
  fromDisplay,
  getNeutral,
  formatVal,
  temperatureToKelvin,
  kelvinToTemperature,
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

  it("shows the pre-film local contrast as a percentage of a recipe value between 0 and 1", () => {
    expect(toDisplay("film_prep", 0.6)).toBeCloseTo(60);
    expect(fromDisplay("film_prep", 60)).toBeCloseTo(0.6);
    expect(fromDisplay("film_prep", toDisplay("film_prep", 0.35))).toBeCloseTo(0.35);
    expect(formatVal("film_prep", 0)).toBe("Off");
    expect(formatVal("film_prep", 60)).toBe("60%");
    // And the others are untouched.
    expect(toDisplay("exposure_ev", 0.6)).toBe(0.6);
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

describe("temperature in kelvin", () => {
  it("shows the kelvin the slider means", () => {
    expect(formatVal("temperature", 0)).toBe("5500 K");
    expect(formatVal("temperature", 10)).toBe("5950 K");
    expect(formatVal("temperature", -10)).toBe("5150 K");
  });

  it("turns a typed kelvin back into the slider value", () => {
    for (const v of [-100, -37.5, 0, 12, 50, 100]) {
      expect(kelvinToTemperature(temperatureToKelvin(v))).toBeCloseTo(v, 6);
    }
    expect(kelvinToTemperature(6400)).toBeCloseTo(20, 6);
  });
});
