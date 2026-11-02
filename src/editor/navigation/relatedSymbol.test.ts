import { describe, expect, it, vi } from "vitest";
import { switchToRelated } from "./relatedSymbol";

function deps(files: string[]) {
  return {
    related: vi.fn(async () => files),
    open: vi.fn(async () => {}),
    hint: vi.fn(),
  };
}

describe("switch to related source / header", () => {
  it("opens the first related file", async () => {
    const d = deps(["/p/include/greeter.h"]);
    await expect(switchToRelated("/p/src/greeter.cpp", d)).resolves.toBe(true);
    expect(d.open).toHaveBeenCalledWith("/p/include/greeter.h");
    expect(d.hint).not.toHaveBeenCalled();
  });

  it("explains when a file has no counterpart", async () => {
    const d = deps([]);
    await expect(switchToRelated("/p/src/lonely.cpp", d)).resolves.toBe(false);
    expect(d.open).not.toHaveBeenCalled();
    expect(d.hint).toHaveBeenCalledWith("No header or source with the same name was found for lonely.cpp.");
  });

  it("names the file with Windows separators too", async () => {
    const d = deps([]);
    await switchToRelated("C:\\p\\src\\lonely.cpp", d);
    expect(d.hint).toHaveBeenCalledWith("No header or source with the same name was found for lonely.cpp.");
  });
});
