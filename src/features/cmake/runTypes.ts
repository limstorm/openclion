/**
 * "CMake Application" in Run › Edit Configurations › Add New Configuration,
 * and the form of the executable-target configurations `ide-run` detects
 * (`kind: "native"`): the built artifact, the build directory holding it and
 * the program arguments. Debugging uses lldb-dap, like a detected target.
 */
import type { RunConfig } from "@/platform/ipc";
import { formatArgs, parseArgs } from "@/features/run/configs";
import { registerRunConfigType, type RunConfigTypeContribution } from "@/workbench/contributions";

/** Build directories known to the CMake configurations, for the suggestion list. */
export function buildDirs(configs: readonly RunConfig[]): string[] {
  const dirs = new Set<string>();
  for (const c of configs) {
    if (c.kind !== "cmake") continue;
    const fromParams = c.params?.buildDir;
    if (fromParams) dirs.add(fromParams);
    const build = c.args.indexOf("--build");
    if (build >= 0 && c.args[build + 1]) dirs.add(c.args[build + 1]!);
  }
  return [...dirs].sort();
}

/** Executables of the detected target configurations, for the executable suggestion list. */
function executables(configs: readonly RunConfig[]): string[] {
  return [...new Set(configs.filter((c) => c.kind === "native" && c.command).map((c) => c.command))];
}

/** `acme` in `cmake-build-debug` runs `cmake-build-debug/acme` relative to the project root. */
function resolveProgram(program: string, buildDir: string): string {
  const dir = buildDir.replace(/[\\/]+$/, "");
  if (!program || !dir || program.includes("/") || program.includes("\\")) return program;
  return `${dir}/${program}`;
}

export const cmakeApplication: RunConfigTypeContribution = {
  id: "native",
  label: "CMake Application",
  icon: "file-cmake",
  group: "CMake",
  description: "Run an executable target built by CMake",
  debugAdapter: "lldb-dap",
  fields: [
    {
      key: "program",
      label: "Executable",
      mono: true,
      required: true,
      placeholder: "cmake-build-debug/app, or an absolute path",
      hint: "A bare name is looked up in the build directory; detected targets are absolute.",
      suggestions: executables,
    },
    {
      key: "buildDir",
      label: "Build directory",
      mono: true,
      placeholder: "cmake-build-debug",
      suggestions: buildDirs,
    },
    { key: "arguments", label: "Program arguments", mono: true },
  ],
  commandLine: (p) => ({
    command: resolveProgram((p.program ?? "").trim(), (p.buildDir ?? "").trim()),
    args: parseArgs(p.arguments ?? ""),
  }),
  paramsFrom: (c) => {
    if (!c.command) return null;
    const buildDir = c.params?.buildDir ?? "";
    return { program: c.command, buildDir, arguments: formatArgs(c.args) };
  },
};

export function registerCmakeRunTypes(): () => void {
  return registerRunConfigType(cmakeApplication);
}
