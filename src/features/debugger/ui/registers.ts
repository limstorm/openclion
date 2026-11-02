/**
 * Registers of a paused frame: DAP has no "registers" request — adapters expose
 * them as a variable scope whose `presentationHint` is `registers` (lldb-dap,
 * CodeLLDB and gdb do). These helpers pick that scope out and shape its
 * variables for the view.
 */
import type { Scope, Variable } from "@/platform/ipc";

/** The scope holding CPU registers, when the adapter exposes one. */
export function registerScope(scopes: readonly Scope[]): Scope | undefined {
  return scopes.find((s) => s.presentationHint === "registers");
}

export interface RegisterRow {
  name: string;
  value: string;
  /** `type` the adapter reports, when it does (e.g. `uint64_t`). */
  type: string | null;
}

/**
 * Register rows from the scope's variables, in adapter order. Vector registers
 * (`xmm0`) come with their own sub-fields; their textual value is enough here.
 */
export function registerRows(variables: readonly Variable[]): RegisterRow[] {
  return variables.map((v) => ({ name: v.name, value: v.value, type: v.typeName ?? null }));
}

/** `0x00007ffe…` values read better right-aligned; long ones are left as the adapter wrote them. */
export function formatRegisterValue(value: string): string {
  return value.trim();
}

/** Which registers changed since the last frame: the ones worth highlighting. */
export function changedRegisters(previous: readonly RegisterRow[], next: readonly RegisterRow[]): ReadonlySet<string> {
  if (previous.length === 0) return new Set();
  const before = new Map(previous.map((r) => [r.name, r.value]));
  return new Set(next.filter((r) => before.has(r.name) && before.get(r.name) !== r.value).map((r) => r.name));
}
