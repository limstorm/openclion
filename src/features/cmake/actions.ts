/**
 * Actions of the CMake tool window. Every one of them is a CMake run
 * configuration: the tool window looks the configuration up and hands it to
 * the Run feature, so builds, before-launch ordering, output, exit codes and
 * the Run tool window all behave exactly as when the user runs it directly.
 */
import { executeAction } from "@/platform/actions";
import { tx } from "@/platform/i18n";
import type { CmakeBuild, CmakeTarget, RunConfig } from "@/platform/ipc";
import { notifications } from "@/workbench/contributions";
import { loadConfigs, useRun } from "@/features/run/store";
import { buildConfig, configureConfig, targetBuildConfig, targetRunConfig, testConfig } from "./configs";

async function detectedConfigs(): Promise<readonly RunConfig[]> {
  if (!useRun.getState().loaded) await loadConfigs();
  return useRun.getState().configs;
}

function notFound(what: string, dir: string): undefined {
  notifications().notify({
    group: "cmake",
    level: "warning",
    title: "CMake configuration not found",
    body: tx`There is no run configuration for ${what} in ${dir}. Check the CMake profile in Settings › Build, Execution, Deployment › CMake.`,
    key: "cmake-missing-configuration",
  });
  return undefined;
}

async function run(config: RunConfig | undefined, debug: boolean): Promise<void> {
  if (!config) return;
  await executeAction(debug ? "run.debugConfig" : "run.runConfig", config.id);
}

/** `cmake -S . -B <dir>`: (re)generates the build system and the codemodel. */
export async function reload(build: CmakeBuild): Promise<void> {
  const configs = await detectedConfigs();
  await run(configureConfig(configs, build.dir) ?? notFound("configuring the project", build.dir), false);
}

/** `cmake --build <dir>`: every target of the build directory. */
export async function buildAll(build: CmakeBuild): Promise<void> {
  const configs = await detectedConfigs();
  await run(buildConfig(configs, build.dir) ?? notFound("building the project", build.dir), false);
}

/** `cmake --build <dir> --target <target>`. */
export async function buildTarget(build: CmakeBuild, target: CmakeTarget): Promise<void> {
  const configs = await detectedConfigs();
  await run(targetBuildConfig(configs, build, target) ?? notFound(`building '${target.name}'`, build.dir), false);
}

/** Starts an executable target; its build runs first as a before-launch task. */
export async function runTarget(target: CmakeTarget): Promise<void> {
  const configs = await detectedConfigs();
  await run(targetRunConfig(configs, target) ?? notFound(`running '${target.name}'`, target.artifact ?? ""), false);
}

/** Same, under lldb-dap. */
export async function debugTarget(target: CmakeTarget): Promise<void> {
  const configs = await detectedConfigs();
  await run(targetRunConfig(configs, target) ?? notFound(`debugging '${target.name}'`, target.artifact ?? ""), true);
}

/** `ctest --test-dir <dir>`: registers are reported in the test tree. */
export async function runTests(build: CmakeBuild): Promise<void> {
  const configs = await detectedConfigs();
  await run(testConfig(configs, build.dir) ?? notFound("running tests", build.dir), false);
}
