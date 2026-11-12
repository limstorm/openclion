import { describe, expect, it } from "vitest";
import type { Sdk } from "@/bindings";
import { interpreterChoices, isPythonPath } from "../languages/PythonStatus";

describe("Python interpreter detection", () => {
  it.each([
    "src/main.py",
    "types/lib.pyi",
    "C:\\app\\main.pyw",
    "pyproject.toml",
    "requirements-dev.txt",
    "SConstruct",
  ])("recognizes %s", (path) => expect(isPythonPath(path)).toBe(true));

  it.each(["main.ts", "python.md", "pyproject.toml.bak", "copy.py.json"])("rejects %s", (path) =>
    expect(isPythonPath(path)).toBe(false),
  );

  it("keeps custom interpreters and deduplicates detected paths", () => {
    const sdk = { path: "/python", name: "Python 3.12" } as Sdk;
    expect(interpreterChoices([sdk, sdk], "/custom")).toEqual([sdk, { path: "/custom", name: "/custom" }]);
  });
});
