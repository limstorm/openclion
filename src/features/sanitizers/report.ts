/**
 * Sanitizer reports on stderr (ASan, LeakSanitizer, UBSan, TSan): the IDE does
 * not run these tools itself — they are compiled into the binary (`-fsanitize=…`)
 * and print when a check fires. This module reads the run's console text and
 * finds the reports, so the findings can reach the Problems view with their
 * source lines instead of scrolling by in the console.
 */
import type { Diagnostic } from "@/platform/ipc";

export type SanitizerTool = "AddressSanitizer" | "LeakSanitizer" | "ThreadSanitizer" | "UndefinedBehaviorSanitizer";

export interface SanitizerFrame {
  function: string | null;
  file: string | null;
  line: number | null;
}

export interface SanitizerFinding {
  tool: SanitizerTool;
  /** `heap-buffer-overflow`, `detected memory leaks`, `signed integer overflow`… */
  kind: string;
  message: string;
  frames: SanitizerFrame[];
  /** The innermost frame with a file and a line. */
  primary: SanitizerFrame | null;
}

/** `==12345==ERROR: AddressSanitizer: heap-buffer-overflow on address …` */
const HEADER = /^==\d+==ERROR: (AddressSanitizer|LeakSanitizer|ThreadSanitizer):\s*(.*)$/;
/** `src/main.cpp:10:12: runtime error: signed integer overflow: …` (UBSan, no banner). */
const UBSAN = /^\s*(\S[^:\s]*\.(?:c|cc|cpp|cxx|h|hpp|m|mm)):(\d+):(\d+): runtime error:\s*(.*)$/;
/** `  #0 0x5555 in main /home/me/src/main.cpp:12:9` and `  #1 0x… in fn <unknown>:0`. */
const FRAME = /^\s*#(\d+) 0x[0-9a-fA-F]+ in (.+?) (\S+?):(\d+)(?::\d+)?$/;
/** LeakSanitizer's `Direct leak of 40 byte(s) in 1 object(s) allocated from:`. */
const LEAK = /^(Direct|Indirect) leak of \d+ byte\(s\)/;

function sanitizerTool(name: string): SanitizerTool {
  return name as SanitizerTool;
}

/** The stack frames that follow a report header (until a blank line or a new banner). */
function readFrames(lines: readonly string[], from: number): { frames: SanitizerFrame[]; next: number } {
  const frames: SanitizerFrame[] = [];
  let i = from;
  for (; i < lines.length; i++) {
    const line = lines[i]!;
    const frame = FRAME.exec(line);
    if (frame) {
      frames.push({ function: frame[2] ?? null, file: frame[3] ?? null, line: Number(frame[4]) });
      continue;
    }
    if (line.trim() === "" || HEADER.test(line) || UBSAN.test(line)) break;
  }
  return { frames, next: i };
}

/**
 * Findings in a run's console text. Reports are recognized by their banners
 * (ASan/LSan/TSan) and by UBSan's `path:line:col: runtime error:` lines.
 */
export function parseSanitizerOutput(text: string): SanitizerFinding[] {
  const lines = text.split("\n");
  const findings: SanitizerFinding[] = [];
  let leakKind: string | null = null;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    const header = HEADER.exec(line);
    if (header) {
      const tool = sanitizerTool(header[1]!);
      const rest = header[2]!.trim();
      const { frames, next } = readFrames(lines, i + 1);
      findings.push({
        tool,
        kind: rest.split(/\s+/).slice(0, 1).join(" ") || "error",
        message: rest,
        frames,
        primary: frames.find((f) => f.file && f.line) ?? null,
      });
      i = next - 1;
      leakKind = null;
      continue;
    }
    const ubsan = UBSAN.exec(line);
    if (ubsan) {
      const frame = { function: null, file: ubsan[1]!, line: Number(ubsan[2]) };
      findings.push({
        tool: "UndefinedBehaviorSanitizer",
        kind: ubsan[4]!.split(":")[0] ?? "runtime error",
        message: ubsan[4]!,
        frames: [frame],
        primary: frame,
      });
      continue;
    }
    // LeakSanitizer prints one "Direct leak of …" block per leak before the frames.
    if (LEAK.test(line)) {
      leakKind = line.split(" of ")[0] ?? "leak";
      const { frames, next } = readFrames(lines, i + 1);
      findings.push({
        tool: "LeakSanitizer",
        kind: leakKind,
        message: line.trim(),
        frames,
        primary: frames.find((f) => f.file && f.line) ?? null,
      });
      i = next - 1;
      continue;
    }
  }
  return findings;
}

/** One line for a notification: "3 findings (1 leak, 2 memory)". */
export function describeSanitizerFindings(findings: readonly SanitizerFinding[]): string {
  if (findings.length === 0) return "No sanitizer report";
  const byTool = new Map<string, number>();
  for (const f of findings) byTool.set(f.tool, (byTool.get(f.tool) ?? 0) + 1);
  const parts = [...byTool].map(([tool, count]) => `${count} ${tool}`);
  return `${findings.length} finding${findings.length === 1 ? "" : "s"} (${parts.join(", ")})`;
}

/**
 * Diagnostics per file, one per finding at its innermost project frame; frames
 * outside the project or in a build directory are ignored for placement.
 */
export function sanitizerDiagnostics(
  findings: readonly SanitizerFinding[],
  projectRoot: string,
): Map<string, Diagnostic[]> {
  const byFile = new Map<string, Diagnostic[]>();
  for (const finding of findings) {
    const frame = finding.primary;
    if (!frame?.file || frame.line === null) continue;
    if (!frame.file.startsWith(projectRoot)) continue;
    if (/[\\/](cmake-build-[^\\/]+|[^\\/]*build|out)[\\/]/.test(frame.file.slice(projectRoot.length))) continue;
    const line = Math.max(0, frame.line - 1);
    const diagnostic: Diagnostic = {
      range: { start: { line, col: 0 }, end: { line, col: 0 } },
      severity: "error",
      message: `${finding.tool}: ${finding.message}`,
      source: "sanitizer",
      code: finding.kind,
      server: null,
    };
    const list = byFile.get(frame.file);
    if (list) list.push(diagnostic);
    else byFile.set(frame.file, [diagnostic]);
  }
  return byFile;
}
