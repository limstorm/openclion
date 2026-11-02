import { describe, expect, it } from "vitest";
import { describeSanitizerFindings, parseSanitizerOutput, sanitizerDiagnostics } from "./report";

const ASAN = `==12345==ERROR: AddressSanitizer: heap-buffer-overflow on address 0x6020000000f4 at pc 0x55
READ of size 4 at 0x6020000000f4 thread T0
    #0 0x5555555552a1 in sum /home/me/acme/src/vector.cpp:42:9
    #1 0x555555555100 in main /home/me/acme/src/main.cpp:12:5
    #2 0x7ffff7c29d8f in __libc_start_call_main ../sysdeps/nptl/libc_start_call_main.h:58:16

SUMMARY: AddressSanitizer: heap-buffer-overflow /home/me/acme/src/vector.cpp:42:9 in sum
`;

const UBSAN = `/home/me/acme/src/math.cpp:10:12: runtime error: signed integer overflow: 2147483647 + 1 cannot be represented in type 'int'
`;

const LSAN = `==4242==ERROR: LeakSanitizer: detected memory leaks

Direct leak of 40 byte(s) in 1 object(s) allocated from:
    #0 0x7f in malloc (/lib/x86_64-linux-gnu/libasan.so.6+0x1)
    #1 0x5555 in make_buffer /home/me/acme/src/buffer.cpp:7:15

SUMMARY: AddressSanitizer: 40 byte(s) leaked in 1 allocation(s).
`;

describe("sanitizer reports", () => {
  it("reads an AddressSanitizer report with its stack", () => {
    const findings = parseSanitizerOutput(ASAN);
    expect(findings).toHaveLength(1);
    expect(findings[0]!.tool).toBe("AddressSanitizer");
    expect(findings[0]!.kind).toBe("heap-buffer-overflow");
    expect(findings[0]!.frames.map((f) => f.function)).toEqual(["sum", "main", "__libc_start_call_main"]);
    expect(findings[0]!.primary).toEqual({ function: "sum", file: "/home/me/acme/src/vector.cpp", line: 42 });
  });

  it("reads UBSan's path:line:col runtime errors", () => {
    const findings = parseSanitizerOutput(UBSAN);
    expect(findings).toHaveLength(1);
    expect(findings[0]!.tool).toBe("UndefinedBehaviorSanitizer");
    expect(findings[0]!.message).toContain("signed integer overflow");
    expect(findings[0]!.primary).toEqual({ function: null, file: "/home/me/acme/src/math.cpp", line: 10 });
  });

  it("reads LeakSanitizer's leak blocks", () => {
    const findings = parseSanitizerOutput(LSAN);
    expect(findings.map((f) => f.tool)).toEqual(["LeakSanitizer", "LeakSanitizer"]);
    expect(findings[1]!.kind).toBe("Direct leak");
    expect(findings[1]!.primary).toEqual({ function: "make_buffer", file: "/home/me/acme/src/buffer.cpp", line: 7 });
  });

  it("ignores ordinary program output", () => {
    expect(parseSanitizerOutput("hello\n42\n")).toEqual([]);
    expect(describeSanitizerFindings([])).toBe("No sanitizer report");
  });

  it("describes what was found", () => {
    expect(describeSanitizerFindings(parseSanitizerOutput(ASAN))).toBe("1 finding (1 AddressSanitizer)");
  });

  it("places diagnostics at the innermost project frame", () => {
    const diagnostics = sanitizerDiagnostics(parseSanitizerOutput(ASAN + UBSAN), "/home/me/acme");
    expect([...diagnostics.keys()].sort()).toEqual(["/home/me/acme/src/math.cpp", "/home/me/acme/src/vector.cpp"]);
    expect(diagnostics.get("/home/me/acme/src/vector.cpp")?.[0]).toMatchObject({
      severity: "error",
      code: "heap-buffer-overflow",
      source: "sanitizer",
      range: { start: { line: 41, col: 0 } },
    });
  });

  it("skips frames outside the project and in build directories", () => {
    const text = `==1==ERROR: AddressSanitizer: heap-use-after-free on address 0x1
    #0 0x1 in lib /usr/lib/libfoo.so:3:1
    #1 0x2 in built /home/me/acme/cmake-build-debug/gen.cpp:9:1
`;
    expect(parseSanitizerOutput(text)).toHaveLength(1);
    expect(sanitizerDiagnostics(parseSanitizerOutput(text), "/home/me/acme").size).toBe(0);
  });
});
