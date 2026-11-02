import { describe, expect, it } from "vitest";
import { describeValgrindReport, parseValgrindReport, valgrindDiagnostics } from "./report";

/** A Memcheck XML report as Valgrind writes it (two errors, one leak). */
const REPORT = `<?xml version="1.0"?>
<valgrindoutput>
 <protocolversion>4</protocolversion>
 <pid>4242</pid>
 <tool>memcheck</tool>
 <status><state>RUNNING</state></status>
 <error>
  <unique>0x1</unique>
  <kind>InvalidRead</kind>
  <what>Invalid read of size 4</what>
  <stack>
   <frame><ip>0x10</ip><fn>sum</fn><file>/home/me/acme/src/vector.cpp</file><line>42</line></frame>
   <frame><ip>0x20</ip><fn>main</fn><file>/home/me/acme/src/main.cpp</file><line>12</line></frame>
  </stack>
 </error>
 <error>
  <unique>0x2</unique>
  <kind>Leak_DefinitelyLost</kind>
  <what>40 bytes in 1 blocks are definitely lost in loss record 1 of 1</what>
  <leakedbytes>40</leakedbytes>
  <stack>
   <frame><ip>0x30</ip><fn>make_buffer</fn><file>/home/me/acme/src/buffer.cpp</file><line>7</line></frame>
  </stack>
 </error>
 <status><state>FINISHED</state></status>
</valgrindoutput>`;

describe("Valgrind XML reports", () => {
  it("reads errors, their stacks and the leak total", () => {
    const report = parseValgrindReport(REPORT);
    expect(report.errors).toHaveLength(2);
    expect(report.errors[0]!.kind).toBe("InvalidRead");
    expect(report.errors[0]!.what).toBe("Invalid read of size 4");
    expect(report.errors[0]!.stack.map((f) => f.function)).toEqual(["sum", "main"]);
    expect(report.errors[0]!.primary).toEqual({ file: "/home/me/acme/src/vector.cpp", line: 42, function: "sum" });
    expect(report.errors[1]!.kind).toBe("Leak");
    expect(report.summary.leakedBytes).toBe(40);
    expect(report.summary.errors).toBe(2);
    expect(report.summary.kinds).toEqual([
      { kind: "InvalidRead", count: 1 },
      { kind: "Leak_DefinitelyLost", count: 1 },
    ]);
  });

  it("survives an empty or malformed file", () => {
    expect(parseValgrindReport("").errors).toEqual([]);
    expect(parseValgrindReport("<valgrindoutput><error>").errors).toEqual([]);
    expect(parseValgrindReport("not xml at all").errors).toEqual([]);
  });

  it("summarizes what was found", () => {
    expect(describeValgrindReport(parseValgrindReport(REPORT))).toBe("Valgrind found 1 error and 1 leak record");
    expect(describeValgrindReport(parseValgrindReport("<valgrindoutput/>"))).toBe("Valgrind found no errors");
  });

  it("places diagnostics at the innermost project frame", () => {
    const diagnostics = valgrindDiagnostics(parseValgrindReport(REPORT), "/home/me/acme");
    expect([...diagnostics.keys()].sort()).toEqual(["/home/me/acme/src/buffer.cpp", "/home/me/acme/src/vector.cpp"]);
    expect(diagnostics.get("/home/me/acme/src/vector.cpp")).toEqual([
      {
        range: { start: { line: 41, col: 0 }, end: { line: 41, col: 0 } },
        severity: "error",
        message: "Invalid read of size 4 (in sum)",
        source: "valgrind",
        code: "InvalidRead",
        server: null,
      },
    ]);
    expect(diagnostics.get("/home/me/acme/src/buffer.cpp")?.[0]?.severity).toBe("warning");
  });

  it("ignores frames outside the project and inside build directories", () => {
    const xml = `<valgrindoutput><error><kind>InvalidWrite</kind><what>Invalid write of size 8</what>
      <stack>
        <frame><fn>libc</fn><file>/usr/src/libc.so</file><line>3</line></frame>
        <frame><fn>built</fn><file>/home/me/acme/cmake-build-debug/gen.cpp</file><line>9</line></frame>
      </stack></error></valgrindoutput>`;
    expect(valgrindDiagnostics(parseValgrindReport(xml), "/home/me/acme").size).toBe(0);
  });
});
