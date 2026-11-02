/**
 * Toggle Line Breakpoint: an existing method breakpoint on the line is removed
 * instead, otherwise a line breakpoint is toggled.
 */
import { removeSpecial, specialAt } from "./special";
import { toggleBreakpoint } from "./store";

export function toggleBreakpointSmart(path: string, line: number) {
  const special = specialAt(path).find((b) => b.line === line);
  if (special) {
    removeSpecial(special.id);
    return;
  }
  toggleBreakpoint(path, line);
}
