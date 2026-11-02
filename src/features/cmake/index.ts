/**
 * CMake tool window for C/C++ projects: it appears once the project model
 * reports a `CMakeLists.txt`, and disappears with the project (like the Maven
 * and Gradle windows). It is a reader — configuring, building, running and
 * testing go through the CMake run configurations `ide-run` detects.
 */
import { registerActions } from "@/platform/actions";
import { setContext } from "@/platform/context";
import { onIdeEvent } from "@/platform/ipc";
import { useProject } from "@/platform/project";
import { registerToolWindow } from "@/workbench/contributions";
import { useRun } from "@/features/run/store";
import { buildAll, reload } from "./actions";
import { CMakeHeaderActions, CMakeView } from "./CMakeView";
import { registerCmakeRunTypes } from "./runTypes";
import { loadModel, resetCmake, useCmake } from "./store";

const WHEN = "projectOpen && build.cmake";

/** Files whose change means the CMake model moved: configure output and the project file. */
const CMAKE_FILE = /(?:^|\/)(?:CMakeLists\.txt|CMakePresets\.json|CMakeCache\.txt|compile_commands\.json)$/;

export function activate(): () => void {
  let registered: (() => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const sync = () => {
    const visible = !!useCmake.getState().model?.hasProject;
    setContext("build.cmake", visible);
    if (visible && !registered) {
      registered = registerToolWindow({
        id: "cmake",
        title: "CMake",
        icon: "toolwindow-cmake",
        anchor: "right",
        order: 12,
        component: CMakeView,
        headerActions: CMakeHeaderActions,
      });
    } else if (!visible && registered) {
      registered();
      registered = null;
    }
  };

  const scheduleLoad = (delay = 250) => {
    clearTimeout(timer);
    timer = setTimeout(() => void loadModel(), delay);
  };

  /** The build directory an action applies to: the first configured one, else the first. */
  const currentBuild = () => {
    const builds = useCmake.getState().model?.builds ?? [];
    return builds.find((b) => b.configured) ?? builds[0];
  };

  const disposers: Array<() => void> = [
    registerCmakeRunTypes(),
    useCmake.subscribe(sync),
    registerActions([
      {
        id: "cmake.reloadProject",
        title: "Reload CMake Project",
        description: "Runs CMake to regenerate the build system of the project",
        category: "CMake",
        icon: "refresh",
        when: WHEN,
        keywords: ["cmake", "configure", "reload", "reconfigure", "regenerate"],
        run: async () => {
          const build = currentBuild();
          if (build) await reload(build);
        },
      },
      {
        id: "cmake.buildProject",
        title: "Build CMake Project",
        description: "Builds every target of the project's build directory",
        category: "CMake",
        icon: "build-project",
        when: WHEN,
        keywords: ["cmake", "build", "make", "compile"],
        run: async () => {
          const build = currentBuild();
          if (build) await buildAll(build);
        },
      },
    ]),
    useProject.subscribe((s, prev) => {
      if (s.project?.root === prev.project?.root) return;
      resetCmake();
      if (s.project) scheduleLoad(0);
    }),
    onIdeEvent("fsChanged", (e) => {
      if ([...e.created, ...e.modified, ...e.removed].some((p) => CMAKE_FILE.test(p))) scheduleLoad();
    }),
    onIdeEvent("runExited", (e) => {
      if (useRun.getState().sessions.some((s) => s.runId === e.run && s.config.kind === "cmake")) scheduleLoad();
    }),
    onIdeEvent("settingsChanged", (e) => {
      if (e.key === "cmake" || e.key.startsWith("cmake.")) scheduleLoad();
    }),
  ];

  if (useProject.getState().project) scheduleLoad(0);

  return () => {
    clearTimeout(timer);
    registered?.();
    disposers.reverse().forEach((d) => d());
  };
}
