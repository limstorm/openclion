/**
 * Rows of the CMake tool window: the project, one subtree per build directory
 * (a CMake profile when profiles are configured), its targets, the compile
 * database clangd reads and the presets of `CMakePresets.json`.
 *
 * The window only reads the model: configuring and building go through the
 * CMake run configurations `ide-run` detects, so they report in Run / Build
 * like any other command (see `configs.ts`).
 */
import type { CmakeBuild, CmakeModel, CmakePreset, CmakeTarget, CmakeTargetKind } from "@/platform/ipc";

export type CmakeRow =
  | {
      kind: "project";
      key: string;
      label: string;
      builds: readonly CmakeBuild[];
      presets: readonly CmakePreset[];
    }
  | { kind: "build"; key: string; build: CmakeBuild }
  | {
      kind: "group";
      key: string;
      label: string;
      /** The build directory the group belongs to; empty for the project-wide presets. */
      dir: string;
      build?: CmakeBuild;
      presets?: readonly CmakePreset[];
    }
  | { kind: "target"; key: string; build: CmakeBuild; target: CmakeTarget }
  | { kind: "preset"; key: string; preset: CmakePreset }
  | { kind: "file"; key: string; label: string; path: string };

const LAST_SEGMENT = /[\\/]([^\\/]+)$/;

function base(path: string): string {
  return LAST_SEGMENT.exec(path)?.[1] ?? path;
}

/** The row label of a build directory: its profile name, else the directory name. */
export function buildLabel(build: CmakeBuild): string {
  return build.profile ?? (base(build.dir) || build.dir);
}

/** The root of the tree: the CMake project. */
export function top(model: CmakeModel): CmakeRow[] {
  if (!model.hasProject) return [];
  return [
    { kind: "project", key: "project", label: base(model.sourceDir), builds: model.builds, presets: model.presets },
  ];
}

export function children(row: CmakeRow): CmakeRow[] {
  switch (row.kind) {
    case "project": {
      const builds = row.builds.map<CmakeRow>((build) => ({ kind: "build", key: `build:${build.dir}`, build }));
      const presets: CmakeRow[] =
        row.presets.length > 0
          ? [{ kind: "group", key: "group:presets", label: "Presets", dir: "", presets: row.presets }]
          : [];
      return [...builds, ...presets];
    }
    case "build": {
      const out: CmakeRow[] = [];
      if (row.build.targets.length > 0) {
        out.push({
          kind: "group",
          key: `group:targets:${row.build.dir}`,
          label: "Targets",
          dir: row.build.dir,
          build: row.build,
        });
      }
      if (row.build.compileCommands) {
        out.push({
          kind: "file",
          key: `file:compile-commands:${row.build.dir}`,
          label: "compile_commands.json",
          path: row.build.compileCommands,
        });
      }
      return out;
    }
    case "group": {
      if (row.presets) {
        return row.presets.map((preset) => ({ kind: "preset", key: `preset:${preset.name}`, preset }));
      }
      const build = row.build;
      if (!build) return [];
      return build.targets.map((target) => ({
        kind: "target",
        key: `target:${build.dir}:${target.name}`,
        build,
        target,
      }));
    }
    default:
      return [];
  }
}

export function isExpandable(row: CmakeRow): boolean {
  switch (row.kind) {
    case "project":
      return row.builds.length > 0 || row.presets.length > 0;
    case "build":
      return row.build.targets.length > 0 || row.build.compileCommands != null;
    case "group":
      return true;
    default:
      return false;
  }
}

const TARGET_KIND_LABELS: Record<CmakeTargetKind, string> = {
  executable: "executable",
  staticLibrary: "static library",
  sharedLibrary: "shared library",
  moduleLibrary: "module library",
  objectLibrary: "object library",
  interfaceLibrary: "interface library",
  utility: "utility",
  unknown: "target",
};

export function label(row: CmakeRow): string {
  switch (row.kind) {
    case "project":
      return row.label;
    case "build":
      return buildLabel(row.build);
    case "group":
      return row.label;
    case "target":
      return row.target.name;
    case "preset":
      return row.preset.name;
    case "file":
      return row.label;
  }
}

export function icon(row: CmakeRow): string {
  switch (row.kind) {
    case "project":
      return "file-cmake";
    case "build":
      return row.build.configured ? "toolwindow-cmake" : "warning";
    case "group":
      return row.presets ? "file-cmake" : "library";
    case "target":
      return row.target.kind === "executable" ? "file-binary" : "library";
    case "preset":
      return "file-cmake";
    case "file":
      return "file-cmake";
  }
}

/** Muted trailing text, IDEA style: what the row is, or where it writes. */
export function detail(row: CmakeRow): string | undefined {
  switch (row.kind) {
    case "build":
      return row.build.configured ? row.build.dir : `${row.build.dir} — not configured`;
    case "target":
      return TARGET_KIND_LABELS[row.target.kind] === "executable" ? undefined : TARGET_KIND_LABELS[row.target.kind];
    case "preset":
      return row.preset.kind === "build" ? "build preset" : (row.preset.binaryDir ?? "configure preset");
    default:
      return undefined;
  }
}

/** Whether activating the row starts a program (a target, or a build directory with one). */
export function isRunnable(row: CmakeRow): boolean {
  return row.kind === "target" && row.target.kind === "executable";
}
