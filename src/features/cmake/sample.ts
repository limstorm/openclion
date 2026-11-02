/**
 * What the CMake tool window shows for a project without a running backend
 * (plain-browser preview and e2e): the sample CMake project Vite injects —
 * `acme` linking `greeter`, its tests, and the Debug build directory CMake
 * would have configured.
 */
import type { CmakeModel } from "@/platform/ipc";

const ROOT = "/dev/acme-api";
const BUILD = `${ROOT}/build`;

export function sampleModel(): CmakeModel {
  return {
    sourceDir: ROOT,
    hasProject: true,
    hasPresets: false,
    presets: [],
    builds: [
      {
        dir: "build",
        sourceDir: ROOT,
        configured: true,
        generator: "Ninja",
        buildType: "Debug",
        configurationTypes: [],
        compileCommands: `${BUILD}/compile_commands.json`,
        profile: "Debug",
        targets: [
          {
            name: "acme",
            kind: "executable",
            artifact: `${BUILD}/acme`,
            sources: [`${ROOT}/src/main.cpp`, `${ROOT}/src/greeter.cpp`],
            folder: "Apps",
            configuration: "Debug",
          },
          {
            name: "acme_tests",
            kind: "executable",
            artifact: `${BUILD}/acme_tests`,
            sources: [`${ROOT}/tests/test_greeter.cpp`],
            configuration: "Debug",
          },
          {
            name: "greeter",
            kind: "staticLibrary",
            artifact: `${BUILD}/libgreeter.a`,
            sources: [`${ROOT}/src/greeter.cpp`, `${ROOT}/src/util/string_utils.cpp`],
            configuration: "Debug",
          },
        ],
      },
    ],
  };
}
