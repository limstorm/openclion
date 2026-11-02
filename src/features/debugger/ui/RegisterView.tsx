/**
 * Registers view (CLion's "Registers" tab): the CPU registers of the selected
 * frame, read from the debug adapter's `registers` scope. Values that changed
 * since the previous stop are highlighted, like CLion's changed-value colouring.
 */
import { useEffect, useState } from "react";
import { call, commands, type Scope } from "@/platform/ipc";
import { EmptyState, T } from "@/ui";
import { sessionSupports, type DebugSession } from "../state";
import { changedRegisters, registerRows, registerScope, type RegisterRow } from "./registers";
import styles from "./MemoryView.module.css";
import { tx } from "@/platform/i18n";

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

interface Loaded {
  frame: number;
  rows: readonly RegisterRow[];
  changed: ReadonlySet<string>;
  error: string | null;
  /** The adapter exposes no registers scope at all. */
  missing: boolean;
}

export function DebugRegisters({ session }: { session: DebugSession }) {
  const frameId = session.selectedFrame;
  const stoppedAt = `${session.status}:${session.stoppedThread ?? ""}:${frameId ?? ""}`;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const paused = session.status === "paused";
  const canRead = sessionSupports("setValue", session) || session.caps != null;

  useEffect(() => {
    if (!paused || frameId == null) return;
    let live = true;
    void (async () => {
      try {
        const scopes: Scope[] = await call(commands.debugScopes(session.id, frameId));
        const scope = registerScope(scopes);
        if (!scope) {
          if (live) setLoaded({ frame: frameId, rows: [], changed: new Set(), error: null, missing: true });
          return;
        }
        const variables = await call(commands.debugVariables(session.id, scope.variablesReference));
        if (!live) return;
        const rows = registerRows(variables);
        setLoaded((prev) => ({
          frame: frameId,
          rows,
          changed: prev && prev.frame !== frameId ? changedRegisters(prev.rows, rows) : new Set(),
          error: null,
          missing: false,
        }));
      } catch (e) {
        if (live) setLoaded({ frame: frameId, rows: [], changed: new Set(), error: errorText(e), missing: false });
      }
    })();
    return () => {
      live = false;
    };
    // A new stop or frame re-reads; `loaded` is not a dependency on purpose.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stoppedAt, paused, frameId, session.id]);

  if (!canRead) {
    return (
      <EmptyState title="Registers are not available for this debugger">
        This session's debug adapter does not expose a register scope.
      </EmptyState>
    );
  }
  if (!paused) return <div className={styles.note}>{tx("Registers are shown while the debugger is stopped.")}</div>;
  if (loaded?.missing) {
    return (
      <EmptyState title="This debugger does not report registers">
        The adapter has no <code>registers</code> scope for the selected frame.
      </EmptyState>
    );
  }
  if (loaded?.error) return <div className={styles.error}>{loaded.error}</div>;
  if (!loaded) return null;

  return (
    <div className={styles.root}>
      <div className={styles.table} role="table" aria-label="Registers">
        <div className={`${styles.row} ${styles.headerRow}`} role="row">
          <span className={`${styles.cell} ${styles.addressCell} ${styles.header}`} role="columnheader">
            <T>Register</T>
          </span>
          <span className={`${styles.cell} ${styles.instructionCell} ${styles.header}`} role="columnheader">
            <T>Value</T>
          </span>
          <span className={`${styles.cell} ${styles.locationCell} ${styles.header}`} role="columnheader">
            <T>Type</T>
          </span>
        </div>
        {loaded.rows.map((row) => (
          <div
            key={row.name}
            className={`${styles.row} ${loaded.changed.has(row.name) ? styles.changedValue : ""}`}
            role="row"
          >
            <span className={`${styles.cell} ${styles.addressCell}`} role="cell">
              {row.name}
            </span>
            <span className={`${styles.cell} ${styles.instructionCell}`} role="cell">
              {row.value}
            </span>
            <span className={`${styles.cell} ${styles.locationCell}`} role="cell">
              {row.type ?? ""}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
