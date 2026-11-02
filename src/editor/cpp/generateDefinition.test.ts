import { describe, expect, it, vi } from "vitest";
import { isCppHeader, requestDefinition } from "./generateDefinition";
import type { DocId, Pos } from "@/platform/ipc";

const doc = 7 as DocId;
const pos: Pos = { line: 2, col: 9 };

function deps(generated = { file: "/p/src/greeter.cpp", line: 4 }) {
  return {
    generate: vi.fn(async () => generated),
    open: vi.fn(async () => {}),
    hint: vi.fn(),
  };
}

describe("generate definition", () => {
  it("recognizes the C/C++ header extensions", () => {
    for (const path of ["/p/include/greeter.h", "/p/a.hpp", "/p/a.hh", "/p/a.hxx", "/p/a.inl", "A.H"]) {
      expect(isCppHeader(path), path).toBe(true);
    }
    for (const path of ["/p/src/greeter.cpp", "/p/CMakeLists.txt", "/p/README.md"]) {
      expect(isCppHeader(path), path).toBe(false);
    }
  });

  it("opens the source file on the generated stub", async () => {
    const d = deps();
    await expect(requestDefinition("/p/include/greeter.h", doc, pos, d)).resolves.toBe(true);
    expect(d.generate).toHaveBeenCalledWith(doc, pos);
    expect(d.open).toHaveBeenCalledWith("/p/src/greeter.cpp", 4);
    expect(d.hint).not.toHaveBeenCalled();
  });

  it("does nothing in a source file", async () => {
    const d = deps();
    await expect(requestDefinition("/p/src/greeter.cpp", doc, pos, d)).resolves.toBe(false);
    expect(d.generate).not.toHaveBeenCalled();
  });

  it("shows the backend's explanation when there is nothing to generate", async () => {
    const d = deps();
    d.generate.mockRejectedValueOnce(new Error("There is no function declaration here"));
    await expect(requestDefinition("/p/include/greeter.h", doc, pos, d)).resolves.toBe(true);
    expect(d.open).not.toHaveBeenCalled();
    expect(d.hint).toHaveBeenCalledWith("There is no function declaration here");
  });
});
