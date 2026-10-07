import { describe, it, expect, vi, beforeEach } from "vitest";
import { tidyState } from "./tidyState.svelte.js";
import {
  formatPathName,
  formatCount,
  relativePathUnder,
  formatReasons,
  formatFromDirs,
  fetchTidyPlan,
  applyTidyPlan,
} from "./tidyOperations.js";

describe("tidyState", () => {
  beforeEach(() => {
    tidyState.reset();
  });

  it("opens and closes tidy dialog reactively", () => {
    expect(tidyState.isOpen).toBe(false);
    expect(tidyState.dir).toBe(null);

    tidyState.open("/nas/photos/2026-10");
    expect(tidyState.isOpen).toBe(true);
    expect(tidyState.dir).toBe("/nas/photos/2026-10");

    tidyState.close();
    expect(tidyState.isOpen).toBe(false);
    expect(tidyState.dir).toBe(null);
  });
});

describe("tidyOperations", () => {
  it("formatPathName extracts the base directory name", () => {
    expect(formatPathName("/Volumes/Photos/2026/10")).toBe("10");
    expect(formatPathName("/Volumes/Photos/")).toBe("Photos");
    expect(formatPathName("")).toBe("");
  });

  it("formatCount formats numbers with thousands separators", () => {
    expect(formatCount(1234567)).toBe("1,234,567");
    expect(formatCount(42)).toBe("42");
    expect(formatCount(0)).toBe("0");
  });

  it("relativePathUnder calculates relative paths", () => {
    expect(relativePathUnder("/nas/archive/2026/10/photo.jpg", "/nas/archive")).toBe("2026/10/photo.jpg");
    expect(relativePathUnder("/other/path/photo.jpg", "/nas/archive")).toBe("/other/path/photo.jpg");
    expect(relativePathUnder("", "/nas/archive")).toBe("");
  });

  it("formatReasons formats reasons list", () => {
    const reasons = [
      ["already in place", 10],
      ["would move", 25],
    ];
    expect(formatReasons(reasons)).toBe("10 already in place · 25 would move");
    expect(formatReasons([])).toBe("");
  });

  it("formatFromDirs formats source directories list", () => {
    const fromDirs = [
      ["/nas/import/cardA", 5],
      ["/nas/import/cardB", 12],
    ];
    expect(formatFromDirs(fromDirs)).toBe("cardA (5), cardB (12)");
    expect(formatFromDirs([])).toBe("");
  });

  it("fetchTidyPlan invokes tidy_plan command", async () => {
    const invoke = vi.fn().mockResolvedValue({ pattern: "%Y/%m", groups: [] });
    const plan = await fetchTidyPlan("/nas/photos", { invoke });

    expect(invoke).toHaveBeenCalledWith("tidy_plan", { dir: "/nas/photos" });
    expect(plan.pattern).toBe("%Y/%m");

    const empty = await fetchTidyPlan("", { invoke });
    expect(empty).toBe(null);
  });

  it("applyTidyPlan invokes tidy_apply command", async () => {
    const invoke = vi.fn().mockResolvedValue({ moved: 15, errors: [] });
    const res = await applyTidyPlan("/nas/photos", { invoke });

    expect(invoke).toHaveBeenCalledWith("tidy_apply", { dir: "/nas/photos" });
    expect(res.moved).toBe(15);

    const empty = await applyTidyPlan("", { invoke });
    expect(empty).toEqual({ moved: 0, errors: [] });
  });
});
