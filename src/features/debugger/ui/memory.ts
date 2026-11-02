/**
 * The Memory view's arithmetic: turn the address an adapter reports (or an
 * expression evaluated to one) into row addresses, hex cells and the ASCII
 * column. Kept free of React so the formatting is testable.
 */
import type { MemoryBytes } from "@/platform/ipc";

/** Bytes per row, like CLion's default memory view. */
export const BYTES_PER_ROW = 16;
/** One request reads this much: enough for a screenful, small enough for a slow adapter. */
export const PAGE_BYTES = 512;

export interface MemoryRow {
  /** `0x7ffd…` when the address is numeric, else `<address> + <offset>`. */
  address: string;
  /** Cell values; `null` for bytes the adapter could not read. */
  bytes: (number | null)[];
  /** Printable ASCII for the cells, `.` elsewhere. */
  ascii: string;
}

const HEX = /^0x[0-9a-f]+$/i;
const DECIMAL = /^[0-9]+$/;

/** A hex or decimal address, as a number big enough for 64-bit pointers. */
export function parseAddress(text: string): bigint | null {
  const value = text.trim();
  if (HEX.test(value)) return BigInt(value);
  if (DECIMAL.test(value)) return BigInt(value);
  return null;
}

export function formatAddress(value: bigint): string {
  return `0x${value.toString(16)}`;
}

/**
 * The address an expression evaluated to. Adapters answer in their own way:
 * `0x7ffd1234` (lldb-dap, debugpy for pointers), `0x00005555…` or a decimal
 * (`140732…`). Anything else is not an address.
 */
export function addressFromValue(value: string): string | null {
  const trimmed = value.trim().replace(/^\((?:void \*|char \*|.*?\*)\)\s*/, "");
  const first = trimmed.split(/[\s,]/)[0] ?? "";
  const parsed = parseAddress(first);
  return parsed === null ? null : formatAddress(parsed);
}

/** Whether `text` is an address rather than an expression to evaluate. */
export function isAddress(text: string): boolean {
  return parseAddress(text.trim()) !== null;
}

function asciiOf(byte: number | null): string {
  if (byte === null) return ".";
  return byte >= 0x20 && byte <= 0x7e ? String.fromCharCode(byte) : ".";
}

/**
 * Rows of a memory page starting at `start`. The last `unreadableBytes` bytes
 * of the payload become `null` cells (not zeros: they were not read).
 */
export function memoryRows(start: bigint | null, address: string, page: MemoryBytes): MemoryRow[] {
  const readable = page.data.length - page.unreadableBytes;
  const rows: MemoryRow[] = [];
  for (let offset = 0; offset < page.data.length; offset += BYTES_PER_ROW) {
    const bytes: (number | null)[] = [];
    for (let i = 0; i < BYTES_PER_ROW; i++) {
      const index = offset + i;
      bytes.push(index < page.data.length && index < readable ? (page.data[index] ?? null) : null);
    }
    const rowAddress =
      start !== null ? formatAddress(start + BigInt(offset)) : `${address || "?"} + 0x${offset.toString(16)}`;
    rows.push({ address: rowAddress, bytes, ascii: bytes.map(asciiOf).join("") });
  }
  return rows;
}

/** Two-digit uppercase hex, or `??` for an unreadable byte. */
export function hexByte(byte: number | null): string {
  return byte === null ? "??" : byte.toString(16).toUpperCase().padStart(2, "0");
}

/** The page's address for the next / previous request. */
export function shiftedAddress(page: MemoryBytes, deltaBytes: number): string | null {
  const start = parseAddress(page.address);
  if (start === null) return null;
  const next = start + BigInt(deltaBytes);
  return next < 0n ? null : formatAddress(next);
}
