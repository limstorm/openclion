/**
 * CMake tool window (CLion's): the project's build directories — one per CMake
 * profile — their targets, the compile database clangd reads, and the presets.
 * Double-clicking an executable runs it (building first), any other target
 * builds it; the header reloads (configures) and builds the project.
 */
import { useMemo } from "react";
import type { CmakeModel } from "@/platform/ipc";
import { Button, EmptyState, IconButton, Spinner, Tree, showContextMenu, type MenuItem } from "@/ui";
import { editorService } from "@/workbench/contributions";
import { buildAll, buildTarget, debugTarget, reload, runTarget, runTests } from "./actions";
import { children, detail, icon, isExpandable, isRunnable, label, top, type CmakeRow } from "./model";
import { loadModel, useCmake } from "./store";
import styles from "./CMake.module.css";

function openFile(path: string) {
  void editorService().openFile(path, { focus: true });
}

/** Enter / double-click: run an executable, build anything else, open files. */
function activate(row: CmakeRow, model: CmakeModel) {
  switch (row.kind) {
    case "project":
      openFile(`${model.sourceDir}/CMakeLists.txt`);
      return;
    case "build":
      if (row.build.configured) void buildAll(row.build);
      else void reload(row.build);
      return;
    case "group":
      openFile(row.presets ? `${model.sourceDir}/CMakePresets.json` : `${model.sourceDir}/CMakeLists.txt`);
      return;
    case "target":
      if (isRunnable(row)) void runTarget(row.target);
      else void buildTarget(row.build, row.target);
      return;
    case "preset":
      openFile(`${model.sourceDir}/CMakePresets.json`);
      return;
    case "file":
      openFile(row.path);
  }
}

function menuFor(row: CmakeRow, model: CmakeModel): MenuItem[] {
  switch (row.kind) {
    case "build":
      return [
        { label: "Reload CMake Project", run: () => void reload(row.build) },
        { label: "Build", run: () => void buildAll(row.build) },
        { kind: "separator" },
        { label: "Run CTest", run: () => void runTests(row.build) },
      ];
    case "target":
      return [
        ...(isRunnable(row) ? [{ label: "Run", run: () => void runTarget(row.target) }] : []),
        ...(isRunnable(row) ? [{ label: "Debug", run: () => void debugTarget(row.target) }] : []),
        { label: "Build Target", run: () => void buildTarget(row.build, row.target) },
        { kind: "separator" },
        ...(row.target.sources[0] ? [{ label: "Jump to Source", run: () => openFile(row.target.sources[0]!) }] : []),
      ];
    case "file":
      return [{ label: "Open", run: () => openFile(row.path) }];
    default:
      return [{ label: "Open CMakeLists.txt", run: () => openFile(`${model.sourceDir}/CMakeLists.txt`) }];
  }
}

export function CMakeHeaderActions() {
  const model = useCmake((s) => s.model);
  const build = model?.builds.find((b) => b.configured) ?? model?.builds[0];
  return (
    <>
      <IconButton
        icon="refresh"
        size="small"
        title="Reload CMake Project"
        disabled={!build}
        onClick={() => build && void reload(build)}
      />
      <IconButton
        icon="build-project"
        size="small"
        title="Build"
        disabled={!build}
        onClick={() => build && void buildAll(build)}
      />
    </>
  );
}

export function CMakeView() {
  const model = useCmake((s) => s.model);
  const loading = useCmake((s) => s.loading);
  const error = useCmake((s) => s.error);
  const roots = useMemo(() => (model ? top(model) : []), [model]);
  // Targets are open: they are what the window is for.
  const expanded = useMemo(() => {
    const builds = model?.builds ?? [];
    return ["project", ...builds.map((b) => `build:${b.dir}`), ...builds.map((b) => `group:targets:${b.dir}`)];
  }, [model]);

  if (error) {
    return (
      <EmptyState title="CMake model is unavailable" actions={<Button onClick={() => void loadModel()}>Retry</Button>}>
        {error}
      </EmptyState>
    );
  }
  if (!model) return loading ? <Spinner label="Loading CMake model" /> : null;
  if (!model.hasProject) {
    return (
      <EmptyState title="No CMake project">
        This project has no CMakeLists.txt, so there is nothing to configure. Open a project with one to build, run and
        debug its targets from here.
      </EmptyState>
    );
  }

  const buildDir = model.builds.find((b) => b.configured)?.dir ?? model.builds[0]?.dir;
  return (
    <div className={styles.view}>
      <Tree
        className={styles.tree}
        aria-label="CMake"
        roots={roots}
        getKey={(row) => row.key}
        getLabel={label}
        getIcon={(row) => icon(row)}
        getDetail={detail}
        getChildren={(row) => (isExpandable(row) ? children(row) : undefined)}
        isExpandable={isExpandable}
        defaultExpanded={expanded}
        onActivate={(row) => activate(row, model)}
        onContextMenu={(row, _selection, e) => {
          if (row) showContextMenu(e, menuFor(row, model));
        }}
      />
      {buildDir && loading && <div className={styles.status}>Reloading CMake model…</div>}
    </div>
  );
}
