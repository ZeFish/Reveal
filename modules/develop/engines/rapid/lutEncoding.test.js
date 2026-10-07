import { describe, expect, it } from "vitest";
import { LUT_ENCODINGS, lutEncodingOf } from "./lutEncoding.js";

describe("lutEncodingOf", () => {
  it("reads a recipe without the menu as Display", () => {
    expect(lutEncodingOf({})).toBe("display");
    expect(lutEncodingOf(null)).toBe("display");
  });

  it("reads the old LogC switch as LogC3", () => {
    expect(lutEncodingOf({ use_logc: true })).toBe("logc3");
  });

  it("lets the menu win over the old switch", () => {
    expect(lutEncodingOf({ use_logc: true, lut_encoding: "cineon" })).toBe("cineon");
  });
});

describe("LUT_ENCODINGS", () => {
  it("offers the four words the engine knows", () => {
    expect(LUT_ENCODINGS.map((e) => e.value)).toEqual(["display", "logc3", "cineon", "linear"]);
  });
});
