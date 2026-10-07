import { describe, expect, it } from "vitest";
import { MIN_GAP, movePoint } from "./curvePoints.js";

const flat = () => [[0, 0], [1, 1]];

describe("movePoint: the end points", () => {
  it("lifts the black point and moves it to the right", () => {
    const { points, index } = movePoint(flat(), 0, 0.3, 0.1);
    expect(points).toEqual([[0.3, 0.1], [1, 1]]);
    expect(index).toBe(0);
  });

  it("lowers the white point and moves it to the left", () => {
    const { points, index } = movePoint(flat(), 1, 0.7, 0.9);
    expect(points).toEqual([[0, 0], [0.7, 0.9]]);
    expect(index).toBe(1);
  });

  it("cannot pass the point next to it", () => {
    const pts = [[0, 0], [0.4, 0.5], [1, 1]];
    expect(movePoint(pts, 0, 0.9, 0).points[0][0]).toBeCloseTo(0.4 - MIN_GAP);
    expect(movePoint(pts, 2, 0.1, 1).points[2][0]).toBeCloseTo(0.4 + MIN_GAP);
  });

  it("stays inside the square", () => {
    const { points } = movePoint(flat(), 0, -0.5, 2);
    expect(points[0]).toEqual([0, 1]);
  });
});

describe("movePoint: the middle points", () => {
  it("moves freely between the end points", () => {
    const { points, index } = movePoint([[0, 0], [0.5, 0.5], [1, 1]], 1, 0.6, 0.8);
    expect(points[1]).toEqual([0.6, 0.8]);
    expect(index).toBe(1);
  });

  it("cannot pass an end point, even a moved one", () => {
    const pts = [[0.2, 0.1], [0.5, 0.5], [0.8, 0.9]];
    expect(movePoint(pts, 1, 0, 0.5).points[1][0]).toBeCloseTo(0.2 + MIN_GAP);
    expect(movePoint(pts, 1, 1, 0.5).points[1][0]).toBeCloseTo(0.8 - MIN_GAP);
  });

  it("keeps track of itself when it passes another middle point", () => {
    const pts = [[0, 0], [0.3, 0.3], [0.6, 0.6], [1, 1]];
    const { points, index } = movePoint(pts, 1, 0.7, 0.2);
    expect(points.map((p) => p[0])).toEqual([0, 0.6, 0.7, 1]);
    expect(index).toBe(2);
  });
});
