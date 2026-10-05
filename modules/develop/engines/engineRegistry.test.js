import { describe, it, expect } from "vitest";
import {
  registerEngine,
  unregisterEngine,
  getEngineComponent,
  listRegisteredEngines,
} from "./engineRegistry.js";
import GenericEngine from "./GenericEngine.svelte";
import RapidEngine from "./rapid/RapidEngine.svelte";
import SpektraEngine from "./spektra/SpektraEngine.svelte";

describe("engineRegistry", () => {
  it("has rapid and spektra pre-registered", () => {
    expect(getEngineComponent("rapid")).toBe(RapidEngine);
    expect(getEngineComponent("spektra")).toBe(SpektraEngine);

    const list = listRegisteredEngines();
    expect(list.some((e) => e.id === "rapid")).toBe(true);
    expect(list.some((e) => e.id === "spektra")).toBe(true);
  });

  it("falls back to GenericEngine for unknown engines or missing id", () => {
    expect(getEngineComponent("unknown-future-engine")).toBe(GenericEngine);
    expect(getEngineComponent(undefined)).toBe(GenericEngine);
  });

  it("allows registering and unregistering new engines dynamically", () => {
    const customComponent = { name: "CustomEngineMock" };
    registerEngine({
      id: "monochrome",
      label: "Monochrome Lab",
      component: customComponent,
    });

    expect(getEngineComponent("monochrome")).toBe(customComponent);

    unregisterEngine("monochrome");
    expect(getEngineComponent("monochrome")).toBe(GenericEngine);
  });
});
