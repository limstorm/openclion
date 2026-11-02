import { describe, expect, it, vi } from "vitest";
import { requestMembers } from "./generateMembers";
import type { DocId, Pos } from "@/platform/ipc";

const doc = 7 as DocId;
const pos: Pos = { line: 2, col: 9 };

function deps(generated = { file: "/p/include/dog.hpp", line: 5 }) {
  return {
    generate: vi.fn(async () => generated),
    open: vi.fn(async () => {}),
    hint: vi.fn(),
  };
}

describe("generate override/implement methods", () => {
  it("opens the header on the first generated stub", async () => {
    const d = deps();
    await expect(requestMembers("/p/include/dog.hpp", doc, pos, d)).resolves.toBe(true);
    expect(d.generate).toHaveBeenCalledWith(doc, pos);
    expect(d.open).toHaveBeenCalledWith("/p/include/dog.hpp", 5);
    expect(d.hint).not.toHaveBeenCalled();
  });

  it("does nothing in a source file", async () => {
    const d = deps();
    await expect(requestMembers("/p/src/dog.cpp", doc, pos, d)).resolves.toBe(false);
    expect(d.generate).not.toHaveBeenCalled();
    expect(d.open).not.toHaveBeenCalled();
  });

  it("shows the backend's explanation when there is nothing to override", async () => {
    const d = deps();
    d.generate.mockRejectedValueOnce(new Error("Every base-class virtual method is already overridden"));
    await expect(requestMembers("/p/include/dog.hpp", doc, pos, d)).resolves.toBe(true);
    expect(d.open).not.toHaveBeenCalled();
    expect(d.hint).toHaveBeenCalledWith("Every base-class virtual method is already overridden");
  });

  it("opens the header even when the caret is not in a class", async () => {
    const d = deps();
    d.generate.mockRejectedValueOnce(new Error("Put the caret in a C/C++ class to generate its overrides"));
    await requestMembers("/p/include/dog.hpp", doc, pos, d);
    expect(d.hint).toHaveBeenCalledWith("Put the caret in a C/C++ class to generate its overrides");
  });
});
