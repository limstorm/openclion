import { beforeEach, describe, expect, it, vi } from "vitest";

const { toolchainKits } = vi.hoisted(() => ({
  toolchainKits: vi.fn(() =>
    Promise.resolve({
      status: "ok" as const,
      data: [
        {
          id: "/usr/bin/clang",
          name: "Clang 18.1.3",
          family: "clang",
          compiler: "/usr/bin/clang",
          cxxCompiler: "/usr/bin/clang++",
          version: "18.1.3",
          description: "Ubuntu clang version 18.1.3",
          debugger: "/usr/local/bin/lldb-dap",
          cmake: "/usr/bin/cmake",
          buildTool: "/usr/bin/ninja",
        },
      ],
    }),
  ),
}));

vi.mock("@/platform/ipc", async (importOriginal) => {
  const real = await importOriginal<typeof import("@/platform/ipc")>();
  return { ...real, isTauri: () => true, commands: { ...real.commands, toolchainKits } };
});

import { kitCompilers, loadKits, useKits } from "../kits";

describe("compiler kits", () => {
  beforeEach(() => {
    toolchainKits.mockClear();
    useKits.setState({ kits: [], loading: false, loaded: false, error: null });
  });

  it("reads the machine's toolchains once", async () => {
    await loadKits();
    expect(toolchainKits).toHaveBeenCalledTimes(1);
    expect(useKits.getState().kits[0]?.name).toBe("Clang 18.1.3");
    await loadKits();
    expect(toolchainKits).toHaveBeenCalledTimes(1);
    await loadKits(true);
    expect(toolchainKits).toHaveBeenCalledTimes(2);
  });

  it("offers both compilers of every kit for the profile fields", async () => {
    await loadKits();
    const kits = useKits.getState().kits;
    expect(kitCompilers(kits)).toEqual(["/usr/bin/clang", "/usr/bin/clang++"]);
  });
});
