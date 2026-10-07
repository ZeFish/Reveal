import { describe, it, expect } from "vitest";
import { explainTransferError } from "./transferError.js";

describe("explainTransferError", () => {
  it("turns an iCloud error code into what happened and what to try", () => {
    const said = explainTransferError("The operation couldn’t be completed. (CloudPhotoLibraryErrorDomain error 1005.)");
    expect(said).toMatch(/iCloud could not provide this photo/);
    expect(said).not.toMatch(/CloudPhotoLibraryErrorDomain/);
  });

  it("points a permission problem at System Settings", () => {
    expect(explainTransferError("Photos access was denied")).toMatch(/System Settings/);
  });

  it("leaves an error it does not know as it is, and never shows an empty alert", () => {
    expect(explainTransferError("the asset has no resource")).toBe("the asset has no resource");
    expect(explainTransferError("")).toBe("Apple Photos could not provide this photo.");
    expect(explainTransferError(undefined)).toBe("Apple Photos could not provide this photo.");
  });
});
