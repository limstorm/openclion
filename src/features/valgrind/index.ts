/**
 * Valgrind Memcheck integration: a run executor (the "Run with Valgrind" entry
 * next to Run and Debug) and the action that starts it. Findings land in the
 * Problems view with their source lines; the report itself stays in a temporary
 * directory.
 */
import { registerRunExecutor } from "@/workbench/contributions";
import { registerValgrindActions, valgrindExecutor } from "./executor";

export function activate(): () => void {
  const disposers = [registerRunExecutor(valgrindExecutor), registerValgrindActions()];
  return () => disposers.reverse().forEach((d) => d());
}
