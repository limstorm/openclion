import { describe, expect, it } from "vitest";
import type { MemoryBytes } from "@/platform/ipc";
import {
  BYTES_PER_ROW,
  addressFromValue,
  formatAddress,
  hexByte,
  memoryRows,
  parseAddress,
  shiftedAddress,
} from "../ui/memory";

const page = (address: string, data: number[], unreadableBytes = 0): MemoryBytes => ({
  address,
  data,
  unreadableBytes,
});

describe("memory addresses", () => {
  it("parses hex and decimal addresses, rejects expressions", () => {
    expect(parseAddress("0x10")).toBe(16n);
    expect(parseAddress("  0x7ffd1234  ")).toBe(0x7ffd1234n);
    expect(parseAddress("4096")).toBe(4096n);
    expect(parseAddress("&buf")).toBeNull();
    expect(parseAddress("0xZZ")).toBeNull();
  });

  it("reads the address an expression evaluated to", () => {
    expect(addressFromValue("0x7ffd1234")).toBe("0x7ffd1234");
    expect(addressFromValue("(void *) 0x1234")).toBe("0x1234");
    expect(addressFromValue("0x00007ffd1234")).toBe("0x7ffd1234");
    expect(addressFromValue("140732000000000")).toBe(formatAddress(140732000000000n));
    expect(addressFromValue("42")).toBe("0x2a");
    expect(addressFromValue("{...}")).toBeNull();
  });

  it("moves by a page only with a numeric address", () => {
    expect(shiftedAddress(page("0x1000", []), 512)).toBe("0x1200");
    expect(shiftedAddress(page("0x1000", []), -512)).toBe("0xe00");
    expect(shiftedAddress(page("0x100", []), -512)).toBeNull();
    expect(shiftedAddress(page("&buf", []), 512)).toBeNull();
  });
});

describe("memory rows", () => {
  it("lays bytes out in rows with an ASCII column", () => {
    const data = Array.from({ length: BYTES_PER_ROW * 2 }, (_, i) => i + 0x40);
    const rows = memoryRows(0x1000n, "0x1000", page("0x1000", data));
    expect(rows).toHaveLength(2);
    expect(rows[0]!.address).toBe("0x1000");
    expect(rows[1]!.address).toBe("0x1010");
    expect(rows[0]!.bytes[0]).toBe(0x40);
    expect(rows[0]!.ascii.startsWith("@ABC")).toBe(true);
  });

  it("marks unreadable bytes instead of showing zeros", () => {
    const rows = memoryRows(0x10n, "0x10", page("0x10", [0x41, 0x42, 0x00, 0x00], 2));
    expect(rows[0]!.bytes.slice(0, 4)).toEqual([0x41, 0x42, null, null]);
    expect(rows[0]!.ascii.slice(0, 4)).toBe("AB..");
  });

  it("falls back to an offset label for symbolic references", () => {
    const rows = memoryRows(null, "&buffer", page("&buffer", [1, 2]));
    expect(rows[0]!.address).toBe("&buffer + 0x0");
  });

  it("formats cells as two uppercase hex digits", () => {
    expect(hexByte(0)).toBe("00");
    expect(hexByte(0x0a)).toBe("0A");
    expect(hexByte(0xff)).toBe("FF");
    expect(hexByte(null)).toBe("??");
  });
});
