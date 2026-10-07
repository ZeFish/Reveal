import { describe, it, expect } from "vitest";
import { explainAiError } from "./aiError.js";

describe("explainAiError", () => {
  it("turns the provider's 401 into 'check your key', with where to do it", () => {
    const raw = "http: https://api.anthropic.com/v1/messages: status code 401";
    const said = explainAiError(raw);
    expect(said).toMatch(/refused the key/);
    expect(said).toMatch(/Settings → AI & Automation/);
    expect(said).not.toMatch(/https?:/);
  });

  it("treats 403 like 401", () => {
    expect(explainAiError("status code 403")).toMatch(/refused the key/);
  });

  it("says when there is no key at all", () => {
    expect(explainAiError("no vision API key configured (Settings → AI & Automation)")).toMatch(/No AI key yet/);
  });

  it("separates rate limits, provider outages and connection trouble", () => {
    expect(explainAiError("status code 429")).toMatch(/rate-limiting/);
    expect(explainAiError("status code 503")).toMatch(/on its side/);
    expect(explainAiError("dns error: failed to lookup address")).toMatch(/Could not reach/);
  });

  it("leaves an error it does not know as it is, rather than guessing", () => {
    expect(explainAiError("could not read a preview image for this photo")).toBe(
      "could not read a preview image for this photo",
    );
  });

  it("does not mistake a port or a size for a status code", () => {
    expect(explainAiError("read 4010 bytes")).toBe("read 4010 bytes");
  });
});
