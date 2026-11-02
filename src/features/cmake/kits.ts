/**
 * C/C++ toolchains detected on this machine (CLion's "compiler kits"): the
 * compiler pair with the debugger and build tools that go with it. Read once
 * for the settings pages; the browser preview shows a sample kit.
 */
import { create } from "zustand";
import { call, commands, isTauri, type CompilerKit } from "@/platform/ipc";

interface KitsState {
  kits: readonly CompilerKit[];
  loading: boolean;
  loaded: boolean;
  error: string | null;
}

export const useKits = create<KitsState>(() => ({ kits: [], loading: false, loaded: false, error: null }));

/** The sample project's toolchain, for the plain-browser preview. */
const SAMPLE: CompilerKit[] = [
  {
    id: "/usr/bin/clang",
    name: "Clang 18.1.3",
    family: "clang",
    compiler: "/usr/bin/clang",
    cxxCompiler: "/usr/bin/clang++",
    version: "18.1.3",
    description: "Ubuntu clang version 18.1.3 (1ubuntu1)",
    debugger: "/usr/local/bin/lldb-dap",
    cmake: "/usr/bin/cmake",
    buildTool: "/usr/bin/ninja",
  },
];

export async function loadKits(force = false): Promise<void> {
  const state = useKits.getState();
  if (state.loading || (state.loaded && !force)) return;
  if (!isTauri()) {
    useKits.setState({ kits: SAMPLE, loading: false, loaded: true, error: null });
    return;
  }
  useKits.setState({ loading: true, error: null });
  try {
    const kits = await call(commands.toolchainKits());
    useKits.setState({ kits, loading: false, loaded: true, error: null });
  } catch (e) {
    useKits.setState({
      kits: [],
      loading: false,
      loaded: true,
      error: e instanceof Error ? e.message : String(e),
    });
  }
}

/** Compiler paths offered while typing in the CMake profile fields. */
export function kitCompilers(kits: readonly CompilerKit[]): string[] {
  const out = new Set<string>();
  for (const kit of kits) {
    out.add(kit.compiler);
    out.add(kit.cxxCompiler);
  }
  return [...out];
}
