/**
 * Read-only build context shared by file and folder presentation consumers.
 *
 * The Maven/Gradle model is gone with the build tool windows, so there are no
 * build-tool module folders and no Spring detection to share any more; the
 * hook and its shape stay because the project tree and the file icons ask for
 * them.
 */
import { create } from "zustand";

export interface BuildPresentation {
  modules: ReadonlyMap<string, "maven" | "gradle">;
  spring: boolean;
}
const EMPTY: BuildPresentation = { modules: new Map(), spring: false };
export const useBuildPresentation = create<BuildPresentation>(() => EMPTY);

export function useProjectPresentation(): BuildPresentation {
  return EMPTY;
}

export function moduleFolderIcon(rel: string, root: boolean, modules: BuildPresentation["modules"]): string | null {
  const tool = modules.get(rel);
  return tool ? `${tool}-${root ? "project" : "module"}` : null;
}
