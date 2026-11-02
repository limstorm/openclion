/**
 * File icons for the project tree, tab strips and search results.
 *
 * The Java/JVM class-file analysis (reading the first 64 KB of visible `.java` /
 * `.kt` files to pick class/interface/record icons) went away with the Java
 * tooling: the backend no longer exposes `jvm_file_presentations`. The helper
 * names stay because the tree and tabs call them; the icons come from the file
 * name, plus the Spring configuration icon for resource-folder YAML/properties.
 */
import { useMemo } from "react";
import { fileIconName, type IconName } from "@/ui/icons";
import { basename, relativePath, useProject } from "./project";
import { rootFor, type RootFolder } from "./languageSettings";
import { useEffectiveRoots } from "./projectRoots";
import { useProjectPresentation } from "./projectPresentation";

/** Kept for call sites that used the removed JVM presentation. */
export type FilePresentation = null;

/** No cached JVM results any more: nothing to invalidate. */
export function invalidateFilePresentations(_paths: readonly string[]): void {}

export function presentationIcon(
  path: string,
  _info: FilePresentation,
  roots: readonly RootFolder[],
  root: string | null,
  spring: boolean,
): IconName {
  const name = basename(path);
  const role = root ? rootFor(roots, relativePath(root, path))?.kind : null;
  if (
    /^(application|bootstrap).*\.(yml|yaml|properties)$/.test(name) &&
    (spring || role === "resources" || role === "testResources")
  )
    return "spring-config";
  return fileIconName(name);
}

/** Was the JVM presentation of `path`; always empty now. */
export function useJvmPresentation(_path: string, _mtime?: number | null): null {
  return null;
}

export function useFilePresentation(path: string, mtime?: number | null) {
  const info = useJvmPresentation(path, mtime);
  const roots = useEffectiveRoots();
  const root = useProject((s) => s.project?.root ?? null);
  const build = useProjectPresentation();
  return { icon: presentationIcon(path, info, roots, root, build.spring), runnable: false };
}

/** Tab strips resolve their mounted file tabs through the same helper. */
export function useFilePresentationIcons(paths: readonly string[]) {
  const key = JSON.stringify(paths);
  const roots = useEffectiveRoots();
  const root = useProject((s) => s.project?.root ?? null);
  const build = useProjectPresentation();
  return useMemo(
    () =>
      new Map(
        (JSON.parse(key) as string[]).map((path) => [path, presentationIcon(path, null, roots, root, build.spring)]),
      ),
    [key, roots, root, build.spring],
  );
}
