import { describe, it, expect, vi } from "vitest";
import { queueRecipeSave, handleRecipeEdited } from "./developOperations.js";

describe("queueRecipeSave", () => {
  it("hands the recipe to the backend and does nothing else", () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    queueRecipeSave("/a.raf", { engine: "rapid" }, { invoke });
    expect(invoke).toHaveBeenCalledWith("queue_save_recipe", { path: "/a.raf", recipe: { engine: "rapid" } });
  });

  it("tells the person only when the backend could not be reached", async () => {
    const hold = vi.fn();
    queueRecipeSave("/a.raf", {}, { invoke: vi.fn().mockRejectedValue("closed"), hold });
    await Promise.resolve();
    await Promise.resolve();
    expect(hold).toHaveBeenCalledWith("Could not save development settings: closed");
  });
});

describe("handleRecipeEdited", () => {
  it("queues the recipe after a short pause, once for a burst of edits", async () => {
    vi.useFakeTimers();
    try {
      const invoke = vi.fn().mockResolvedValue(undefined);
      const state = /** @type {any} */ ({
        developEngine: "rapid",
        recipe: { engine: "rapid", exposure: 0 },
        recipeUndoStack: [],
        recipeRedoStack: [],
      });
      const options = { photoPath: "/a.raf", liveRenderPx: () => 768, scheduleRender: vi.fn(), invoke, debounceMs: 100 };
      for (const exposure of [0.1, 0.2, 0.3]) {
        state.recipe = { ...state.recipe, exposure };
        handleRecipeEdited(state, true, options);
        vi.advanceTimersByTime(30);
      }
      expect(invoke).not.toHaveBeenCalled();
      vi.advanceTimersByTime(200);
      expect(invoke).toHaveBeenCalledTimes(1);
      expect(invoke).toHaveBeenCalledWith("queue_save_recipe", {
        path: "/a.raf",
        recipe: expect.objectContaining({ exposure: 0.3 }),
      });
    } finally {
      vi.useRealTimers();
    }
  });
});
