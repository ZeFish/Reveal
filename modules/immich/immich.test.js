import { describe, it, expect } from "vitest";
import { photoIdentity } from "@modules/apple-photos";
import { immich, leaveImmich } from "./immich.svelte.js";

describe("immich vertical slice", () => {
  it("photoIdentity strips immich protocol and returns asset id", () => {
    expect(photoIdentity("immich://XYZ-789/thumbnail.jpg")).toBe("XYZ-789");
  });

  it("initial state is inactive", () => {
    expect(immich.active).toBe(false);
  });

  it("leaveImmich increments request and deactivates", () => {
    const reqBefore = immich.request;
    leaveImmich();
    expect(immich.request).toBe(reqBefore + 1);
    expect(immich.active).toBe(false);
    expect(immich.busy).toBe(false);
  });

  it("immich.library computes connection based on preferences", () => {
    const disconnected = immich.library({});
    expect(disconnected.connected).toBe(false);

    const connected = immich.library({
      immich_url: "http://localhost:2283",
      immich_api_key: "secret",
    });
    expect(connected.connected).toBe(true);
  });
});
