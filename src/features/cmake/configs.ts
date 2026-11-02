/**
 * Which run configuration performs a CMake action. `ide-run` derives them from
 * the CMake settings profiles (configure / build per directory, build and run
 * per executable target), so the tool window never invents a command line: it
 * looks the configuration up and runs it like the Run widget would.
 */
import type { CmakeBuild, CmakeTarget, RunConfig } from "@/platform/ipc";

/** `cmake -S . -B <dir>` for the build directory, from the CMake settings profile that owns it. */
export function configureConfig(configs: readonly RunConfig[], dir: string): RunConfig | undefined {
  return configs.find((c) => c.kind === "cmake" && c.id.endsWith(":configure") && c.params?.buildDir === dir);
}

/** `cmake --build <dir>`: everything in the build directory. */
export function buildConfig(configs: readonly RunConfig[], dir: string): RunConfig | undefined {
  return configs.find((c) => c.kind === "cmake" && c.id.endsWith(":build") && c.args.includes(dir));
}

/** `cmake --build <dir> --target <target>`. */
export function targetBuildConfig(
  configs: readonly RunConfig[],
  build: CmakeBuild,
  target: CmakeTarget,
): RunConfig | undefined {
  return configs.find(
    (c) => c.kind === "cmake" && c.id.endsWith(`:${target.name}:build`) && c.args.includes(build.dir),
  );
}

/** The native run configuration of an executable target (artifact built by its own build config). */
export function targetRunConfig(configs: readonly RunConfig[], target: CmakeTarget): RunConfig | undefined {
  if (!target.artifact) return undefined;
  return configs.find((c) => c.kind === "native" && c.command === target.artifact);
}

/** Tests of the build directory (`ctest --test-dir <dir>`), when the project has any. */
export function testConfig(configs: readonly RunConfig[], dir: string): RunConfig | undefined {
  return configs.find((c) => c.kind === "cmake" && c.isTest && c.args.includes(dir));
}
