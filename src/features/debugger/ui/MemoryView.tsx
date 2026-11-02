/**
 * Memory view (CLion's "Memory" tab): a hex dump of an address or of the
 * address an expression evaluates to. Pages of 512 bytes, 16 bytes per row,
 * with the ASCII column; refreshed when the debuggee stops.
 *
 * The adapter must support DAP `readMemory` (lldb-dap, CodeLLDB, debugpy,
 * delve all do); the tab says so instead of showing an empty page otherwise.
 */
import { useEffect, useMemo, useState } from "react";
import { create } from "zustand";
import { call, commands, type MemoryBytes } from "@/platform/ipc";
import { Button, EmptyState, IconButton, Input, T } from "@/ui";
import { evaluate, sessionSupports, type DebugSession } from "../state";
import {
  PAGE_BYTES,
  addressFromValue,
  formatAddress,
  hexByte,
  memoryRows,
  parseAddress,
  shiftedAddress,
} from "./memory";
import styles from "./MemoryView.module.css";
import { tx } from "@/platform/i18n";

interface MemoryViewState {
  /** Last expression the user entered, per session (the tab unmounts on a tab switch). */
  expressions: Record<number, string>;
  page: MemoryBytes | null;
  session: number | null;
  loading: boolean;
  error: string | null;
}

const useMemoryView = create<MemoryViewState>(() => ({
  expressions: {},
  page: null,
  session: null,
  loading: false,
  error: null,
}));

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** The address to read: a literal address, or what the expression evaluates to. */
async function resolveAddress(expression: string, session: DebugSession): Promise<string> {
  const literal = parseAddress(expression);
  if (literal !== null) return formatAddress(literal);
  const variable = await evaluate(expression, "watch", session);
  const address = addressFromValue(variable.value);
  if (!address) throw new Error(tx`'${variable.value}' is not an address`);
  return address;
}

async function readPage(session: DebugSession, address: string, loading: boolean) {
  useMemoryView.setState({ session: session.id, loading, error: null });
  try {
    const page = await call(commands.debugReadMemory(session.id, address, 0, PAGE_BYTES));
    useMemoryView.setState({ page, loading: false, error: null });
  } catch (e) {
    useMemoryView.setState({ page: null, loading: false, error: errorText(e) });
  }
}

/** Reads `expression` (an address or an expression) at `offset` from the last address. */
async function showPage(session: DebugSession, expression: string, offset = 0) {
  useMemoryView.setState((s) => ({ expressions: { ...s.expressions, [session.id]: expression } }));
  let address: string;
  try {
    const resolved = await resolveAddress(expression, session);
    const start = parseAddress(resolved);
    address = start !== null ? formatAddress(start + BigInt(offset)) : resolved;
  } catch (e) {
    useMemoryView.setState({ page: null, loading: false, error: errorText(e) });
    return;
  }
  await readPage(session, address, true);
}

export function DebugMemory({ session }: { session: DebugSession }) {
  const stored = useMemoryView((s) => s.expressions[session.id] ?? "");
  const page = useMemoryView((s) => (s.session === session.id ? s.page : null));
  const loading = useMemoryView((s) => s.session === session.id && s.loading);
  const error = useMemoryView((s) => (s.session === session.id ? s.error : null));
  const [draft, setDraft] = useState(stored || "0x0");
  const supported = sessionSupports("readMemory", session);
  const paused = session.status === "paused";
  const stoppedAt = `${session.status}:${session.stoppedThread ?? ""}`;

  // A new stop means the memory may have changed: re-read the same address.
  useEffect(() => {
    const expression = useMemoryView.getState().expressions[session.id];
    if (!paused || !expression || !supported) return;
    void showPage(session, expression);
    // `stoppedAt` changes on every stop, which is exactly when memory moved.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stoppedAt, session.id, supported]);

  const start = useMemo(() => (page ? parseAddress(page.address) : null), [page]);
  const rows = useMemo(() => (page ? memoryRows(start, page.address, page) : []), [page, start]);

  if (!supported) {
    return (
      <EmptyState title="Memory is not available for this debugger">
        This session's debug adapter does not support reading memory (DAP readMemory).
      </EmptyState>
    );
  }

  const go = () => void showPage(session, draft.trim());
  const shift = (delta: number) => {
    if (!page) return;
    const next = shiftedAddress(page, delta);
    if (next) void readPage(session, next, true);
  };

  return (
    <div className={styles.root}>
      <form
        className={styles.toolbar}
        onSubmit={(e) => {
          e.preventDefault();
          go();
        }}
      >
        <Input
          className={styles.address}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={tx("Address or expression, e.g. 0x7ffd… or &buffer")}
          aria-label="Memory address or expression"
        />
        <Button type="submit" disabled={!paused || loading}>
          <T>Go</T>
        </Button>
        <IconButton
          icon="refresh"
          size="small"
          title="Refresh"
          disabled={!page || !paused || loading}
          onClick={() => page && void readPage(session, page.address, true)}
        />
        <span className={styles.sep} aria-hidden />
        <IconButton
          icon="chevron-up"
          size="small"
          title="Previous 512 bytes"
          disabled={!page || start === null || start < BigInt(PAGE_BYTES)}
          onClick={() => shift(-PAGE_BYTES)}
        />
        <IconButton
          icon="chevron-down"
          size="small"
          title="Next 512 bytes"
          disabled={!page || start === null}
          onClick={() => shift(PAGE_BYTES)}
        />
      </form>
      {!paused && <div className={styles.note}>{tx("Memory is shown while the debugger is stopped.")}</div>}
      {error && <div className={styles.error}>{error}</div>}
      {page && (
        <div className={styles.table} role="table" aria-label="Memory">
          <div className={styles.row} role="row">
            <span className={`${styles.cell} ${styles.addressCell} ${styles.header}`} role="columnheader">
              <T>Address</T>
            </span>
            {Array.from({ length: 16 }, (_, i) => (
              <span key={i} className={`${styles.cell} ${styles.hexCell} ${styles.header}`} role="columnheader">
                {i.toString(16).toUpperCase().padStart(2, "0")}
              </span>
            ))}
            <span className={`${styles.cell} ${styles.ascii} ${styles.header}`} role="columnheader">
              <T>ASCII</T>
            </span>
          </div>
          {rows.map((row) => (
            <div key={row.address} className={styles.row} role="row">
              <span className={`${styles.cell} ${styles.addressCell}`} role="cell">
                {row.address}
              </span>
              {row.bytes.map((byte, i) => (
                <span
                  key={i}
                  className={`${styles.cell} ${styles.hexCell} ${byte === null ? styles.unreadable : ""} ${i === 7 ? styles.gap : ""}`}
                  role="cell"
                >
                  {hexByte(byte)}
                </span>
              ))}
              <span className={`${styles.cell} ${styles.ascii}`} role="cell">
                {row.ascii}
              </span>
            </div>
          ))}
        </div>
      )}
      {!page && paused && !error && <EmptyState title="No memory view" />}
    </div>
  );
}
