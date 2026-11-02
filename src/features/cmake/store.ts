/**
 * The CMake model of the open project (see the `cmake_model` command).
 *
 * Reloaded when CMake writes into a build directory (`fsChanged` on
 * `CMakeCache.txt` / `compile_commands.json`), when a CMake run exits (the
 * tool window's Reload / Build actions run through the Run tool window), and
 * when the CMake settings change (profiles name the build directories).
 */
import { create } from "zustand";
import { call, commands, isTauri, type CmakeModel } from "@/platform/ipc";
import { useProject } from "@/platform/project";
import { sampleModel } from "./sample";

export interface CmakeState {
  model: CmakeModel | null;
  loading: boolean;
  error: string | null;
}

export const useCmake = create<CmakeState>(() => ({ model: null, loading: false, error: null }));

/** Drops responses of a load that a project change or a newer load superseded. */
let generation = 0;

export async function loadModel(): Promise<void> {
  const project = useProject.getState().project;
  const seq = ++generation;
  if (!project) {
    useCmake.setState({ model: null, loading: false, error: null });
    return;
  }
  if (!isTauri()) {
    useCmake.setState({ model: sampleModel(), loading: false, error: null });
    return;
  }
  useCmake.setState({ loading: true });
  try {
    const model = await call(commands.cmakeModel());
    if (seq === generation) useCmake.setState({ model, loading: false, error: null });
  } catch (e) {
    if (seq === generation) {
      useCmake.setState({ model: null, loading: false, error: e instanceof Error ? e.message : String(e) });
    }
  }
}

export function resetCmake(): void {
  generation++;
  useCmake.setState({ model: null, loading: false, error: null });
}
