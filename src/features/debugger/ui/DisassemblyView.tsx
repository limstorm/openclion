/**
 * Disassembly view (CLion's "Disassembly" tab): the instructions around the
 * selected frame's program counter, read through DAP `disassemble`. Instructions
 * with debug info link back to their source line; the current one is marked.
 */
import { useEffect, useMemo, useState } from "react";
import { call, commands, type DisassembledInstruction } from "@/platform/ipc";
import { Button, EmptyState, IconButton, T } from "@/ui";
import { editorService } from "@/workbench/contributions";
import { sessionSupports, type DebugSession } from "../state";
import styles from "./MemoryView.module.css";
import { tx } from "@/platform/i18n";

/** Instructions fetched around the program counter: a screenful above and below. */
const BEFORE = 20;
const COUNT = 120;

interface Loaded {
  reference: string;
  instructions: readonly DisassembledInstruction[];
  error: string | null;
}

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function DebugDisassembly({ session }: { session: DebugSession }) {
  const frame = session.selectedFrame != null ? session.frames.find((f) => f.id === session.selectedFrame) : undefined;
  const reference = frame?.instructionPointer ?? null;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [loading, setLoading] = useState(false);
  const supported = sessionSupports("disassemble", session);
  const paused = session.status === "paused";

  const read = async (memoryReference: string) => {
    setLoading(true);
    try {
      const instructions = await call(commands.debugDisassemble(session.id, memoryReference, -BEFORE, COUNT));
      setLoaded({ reference: memoryReference, instructions, error: null });
    } catch (e) {
      setLoaded({ reference: memoryReference, instructions: [], error: errorText(e) });
    } finally {
      setLoading(false);
    }
  };

  // A new stop or another frame means another program counter.
  useEffect(() => {
    if (!paused || !reference) return;
    if (loaded?.reference === reference) return;
    void read(reference);
    // `loaded` is deliberately not a dependency: only a new PC triggers a read.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paused, reference, session.id]);

  const current = useMemo(() => {
    if (!reference || !loaded) return -1;
    return loaded.instructions.findIndex((i) => i.address.toLowerCase() === reference.toLowerCase());
  }, [loaded, reference]);

  if (!supported) {
    return (
      <EmptyState title="Disassembly is not available for this debugger">
        This session's debug adapter does not support disassembly (DAP disassemble).
      </EmptyState>
    );
  }
  if (!paused) return <div className={styles.note}>{tx("Disassembly is shown while the debugger is stopped.")}</div>;
  if (!reference) {
    return (
      <EmptyState title="No program counter for this frame">
        The adapter did not report an instruction pointer. Select another frame.
      </EmptyState>
    );
  }

  const openLocation = (instruction: DisassembledInstruction) => {
    if (!instruction.file || instruction.line == null) return;
    void editorService().openFile(instruction.file, {
      selection: { line: instruction.line, col: 0 },
      focus: true,
    });
  };

  return (
    <div className={styles.root}>
      <div className={styles.toolbar}>
        <span className={styles.note}>
          <T>Program counter</T>: <code>{reference}</code>
        </span>
        <Button disabled={loading} onClick={() => void read(reference)}>
          <T>Refresh</T>
        </Button>
        <IconButton
          icon="refresh"
          size="small"
          title="Refresh"
          disabled={loading}
          onClick={() => void read(reference)}
        />
      </div>
      {loaded?.error && <div className={styles.error}>{loaded.error}</div>}
      {loaded && (
        <div className={styles.table} role="table" aria-label="Disassembly">
          {loaded.instructions.map((instruction, index) => (
            <div
              key={`${instruction.address}:${index}`}
              className={`${styles.row} ${index === current ? styles.currentInstruction : ""}`}
              role="row"
            >
              <span className={`${styles.cell} ${styles.addressCell}`} role="cell">
                {instruction.address}
              </span>
              <span className={`${styles.cell} ${styles.instructionCell}`} role="cell">
                {instruction.instruction}
              </span>
              <span className={`${styles.cell} ${styles.symbolCell}`} role="cell">
                {instruction.symbol ?? ""}
              </span>
              <span className={`${styles.cell} ${styles.locationCell}`} role="cell">
                {instruction.file && instruction.line != null ? (
                  <button type="button" className={styles.locationLink} onClick={() => openLocation(instruction)}>
                    {instruction.file.split(/[\\/]/).pop()}:{instruction.line + 1}
                  </button>
                ) : null}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
