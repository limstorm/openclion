/**
 * File › New Project…: an empty project folder (no framework templates),
 * optionally a Git repository, opened in a new window (this one when it has
 * no project).
 */
import { useEffect, useState } from "react";
import { create } from "zustand";
import { call, commands, isTauri } from "@/platform/ipc";
import { useProject } from "@/platform/project";
import { useTxTemplate } from "@/platform/i18n";
import { Button, Checkbox, Dialog, Input, Select, T } from "@/ui";
import { defaultProjectDirectory, openProjectInWindow } from "./windows";
import styles from "./app.module.css";

const useNewProject = create<{ open: boolean }>(() => ({ open: false }));
export const showNewProject = () => useNewProject.setState({ open: true });

export function NewProjectOverlay() {
  const open = useNewProject((s) => s.open);
  return open ? <NewProjectDialog onClose={() => useNewProject.setState({ open: false })} /> : null;
}

/** Why `name` cannot be a project folder name, or null. */
export function projectNameError(name: string): string | null {
  const n = name.trim();
  if (!n) return "Enter a project name";
  if (/[/\\:*?"<>|]/.test(n) || n === "." || n === "..")
    return "The name contains characters a folder name cannot have";
  return null;
}

export function joinPath(dir: string, name: string): string {
  const sep = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return dir.endsWith("/") || dir.endsWith("\\") ? `${dir}${name.trim()}` : `${dir}${sep}${name.trim()}`;
}

function errorText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

/** Creates `path` (or reuses it when it is an empty folder) and runs `git init` when asked. */
export async function createProjectFolder(path: string, gitInit: boolean): Promise<void> {
  let entries: unknown[] | null;
  try {
    entries = await call(commands.fsReadDir(path));
  } catch {
    entries = null;
  }
  if (entries && entries.length) throw new Error(`${path} already exists and is not empty`);
  if (!entries) await call(commands.fsCreateDir(path));
  if (gitInit) await call(commands.vcsInit(path));
}

/** Project skeletons File › New Project offers: CLion's CMake templates and an empty folder. */
type ProjectTemplate = "cpp-executable" | "cpp-library" | "empty";

const TEMPLATES: ReadonlyArray<{ value: ProjectTemplate; label: string }> = [
  { value: "cpp-executable", label: "C++ executable (CMake)" },
  { value: "cpp-library", label: "C++ library (CMake)" },
  { value: "empty", label: "Empty project" },
];

/** A CMake target name: spaces and punctuation become `_`. */
export function cmakeTargetName(name: string): string {
  return (
    name
      .trim()
      .replace(/[^A-Za-z0-9_.+-]+/g, "_")
      .replace(/^_+|_+$/g, "") || "app"
  );
}

async function writeFile(path: string, content: string): Promise<void> {
  await call(commands.fsCreateFileBytes(path, [...new TextEncoder().encode(content)]));
}

/** Writes the files of `template` into the freshly created `root`. */
export async function writeProjectTemplate(root: string, name: string, template: ProjectTemplate): Promise<void> {
  if (template === "empty") return;
  const target = cmakeTargetName(name);
  const executable = template === "cpp-executable";
  const sources = executable
    ? `add_executable(${target} src/main.cpp)\n`
    : `add_library(${target} STATIC src/${target}.cpp)\n\ntarget_include_directories(${target} PUBLIC include)\n`;
  await call(commands.fsCreateDir(`${root}/src`));
  if (!executable) await call(commands.fsCreateDir(`${root}/include`));
  await writeFile(
    `${root}/CMakeLists.txt`,
    [
      "cmake_minimum_required(VERSION 3.20)",
      `project(${target} LANGUAGES CXX)`,
      "",
      "set(CMAKE_CXX_STANDARD 20)",
      "set(CMAKE_CXX_STANDARD_REQUIRED ON)",
      "set(CMAKE_EXPORT_COMPILE_COMMANDS ON)",
      "",
      sources,
    ].join("\n"),
  );
  if (executable) {
    await writeFile(
      `${root}/src/main.cpp`,
      [
        "#include <iostream>",
        "",
        "int main(int argc, char** argv) {",
        `    std::cout << "Hello from ${target}!" << std::endl;`,
        "    return 0;",
        "}",
        "",
      ].join("\n"),
    );
  } else {
    await writeFile(
      `${root}/include/${target}.hpp`,
      [
        "#pragma once",
        "",
        "#include <string>",
        "",
        `namespace ${target} {`,
        "",
        "std::string greeting();",
        "",
        "}  // namespace " + target,
        "",
      ].join("\n"),
    );
    await writeFile(
      `${root}/src/${target}.cpp`,
      [
        `#include "${target}.hpp"`,
        "",
        `namespace ${target} {`,
        "",
        "std::string greeting() {",
        `    return "Hello from ${target}!";`,
        "}",
        "",
        "}  // namespace " + target,
        "",
      ].join("\n"),
    );
  }
  await writeFile(`${root}/.gitignore`, "build/\ncmake-build-*/\n");
  await writeFile(`${root}/README.md`, `# ${name}\n\nA CMake ${executable ? "executable" : "library"} project.\n`);
}

/** Default Project Directory, else the parent of the open project. */
function initialLocation(): string {
  const preset = defaultProjectDirectory();
  if (preset) return preset;
  const root = useProject.getState().project?.root;
  return root ? root.slice(0, Math.max(root.lastIndexOf("/"), 1)) : "";
}

function NewProjectDialog({ onClose }: { onClose: () => void }) {
  const [name, setName] = useState("untitled");
  const [location, setLocation] = useState(initialLocation);
  const [git, setGit] = useState(true);
  const [template, setTemplate] = useState<ProjectTemplate>("cpp-executable");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const txTemplate = useTxTemplate();

  useEffect(() => {
    if (initialLocation()) return;
    // Otherwise the user's home.
    void import("@tauri-apps/api/path")
      .then((p) => p.homeDir())
      .then(setLocation)
      .catch(() => {});
  }, []);

  const nameError = projectNameError(name);
  const invalid = !!nameError || !location.trim();
  const path = location.trim() && !nameError ? joinPath(location.trim(), name) : "";

  const browse = async () => {
    if (!isTauri()) return;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({
      directory: true,
      multiple: false,
      title: "Project Location",
      defaultPath: location || undefined,
    });
    const dir = Array.isArray(picked) ? picked[0] : picked;
    if (dir) setLocation(dir);
  };

  const create = async () => {
    if (invalid || busy) return;
    setBusy(true);
    setError(null);
    try {
      await createProjectFolder(path, git);
      await writeProjectTemplate(path, name.trim(), template);
    } catch (e) {
      setBusy(false);
      setError(errorText(e));
      return;
    }
    onClose();
    await openProjectInWindow(path, useProject.getState().project !== null);
  };

  return (
    <Dialog
      open
      title="New Project"
      onClose={onClose}
      width={520}
      buttons={[
        { label: "Create", role: "default", disabled: invalid || busy, onClick: () => void create() },
        { label: "Cancel", role: "cancel" },
      ]}
    >
      <form
        className={styles.newProject}
        onSubmit={(e) => {
          e.preventDefault();
          void create();
        }}
      >
        <div className={styles.newProjectRow}>
          <span className={styles.newProjectLabel}>
            <T>Template:</T>
          </span>
          <Select aria-label="Template" value={template} options={TEMPLATES} onChange={setTemplate} />
        </div>
        <label className={styles.newProjectRow}>
          <span className={styles.newProjectLabel}>
            <T>Name:</T>
          </span>
          <Input
            data-autofocus
            value={name}
            invalid={!!nameError}
            onChange={(e) => setName(e.target.value)}
            aria-label="Name"
          />
        </label>
        <div className={styles.newProjectRow}>
          <span className={styles.newProjectLabel}>
            <T>Location:</T>
          </span>
          <Input value={location} onChange={(e) => setLocation(e.target.value)} aria-label="Location" />
          <Button onClick={() => void browse()} disabled={!isTauri()}>
            <T>Browse…</T>
          </Button>
        </div>
        <div className={styles.newProjectHint}>
          {nameError ? txTemplate(nameError) : path && txTemplate(`Project will be created in: ${path}`)}
        </div>
        <Checkbox checked={git} onChange={setGit} label="Create Git repository" />
        {error && <div className={styles.newProjectError}>{txTemplate(error)}</div>}
      </form>
    </Dialog>
  );
}
