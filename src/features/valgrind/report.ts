/**
 * Valgrind Memcheck reports: `valgrind --xml=yes --xml-file=<file>` writes one
 * `<error>` per finding with the stack it happened on (and per-frame source
 * lines when the binary has debug info), plus leak records and a summary.
 *
 * This module turns that XML into what the IDE can show: one diagnostic per
 * finding at its first project frame, and a summary for the notification.
 */
import { tx } from "@/platform/i18n";
import type { Diagnostic } from "@/platform/ipc";

export type ValgrindErrorKind =
  | "InvalidRead"
  | "InvalidWrite"
  | "InvalidFree"
  | "InvalidJump"
  | "MismatchedFree"
  | "Overlap"
  | "SyscallParam"
  | "UninitCondition"
  | "UninitValue"
  | "Leak"
  | "Other";

export interface ValgrindFrame {
  /** Absolute path the binary reports, usually with the build directory prefix. */
  file: string | null;
  line: number | null;
  function: string | null;
}

export interface ValgrindError {
  kind: ValgrindErrorKind;
  /** The raw `<kind>`, e.g. `Leak_DefinitelyLost`. */
  rawKind: string;
  /** `<what>`, e.g. `Invalid read of size 4` or `40 bytes in 1 blocks are definitely lost`. */
  what: string;
  /** Frames in call order (innermost first). */
  stack: ValgrindFrame[];
  /** Bytes leaked, for leak records (`<leakedbytes>`). */
  leakedBytes: number;
  /** The first frame with both a file and a line: where the diagnostic goes. */
  primary: ValgrindFrame | null;
}

export interface ValgrindSummary {
  /** Sum of `leakedbytes` over the leak records. */
  leakedBytes: number;
  errors: number;
  /** Distinct kinds, most frequent first. */
  kinds: Array<{ kind: string; count: number }>;
}

export interface ValgrindReport {
  errors: ValgrindError[];
  summary: ValgrindSummary;
}

const LEAK_KINDS = /^Leak_/;

function text(node: Element, tag: string): string | null {
  const child = node.getElementsByTagName(tag)[0];
  const value = child?.textContent?.trim();
  return value ? value : null;
}

function number(node: Element, tag: string): number {
  const value = text(node, tag);
  const parsed = value ? Number(value) : Number.NaN;
  return Number.isFinite(parsed) ? parsed : 0;
}

function classify(rawKind: string): ValgrindErrorKind {
  if (LEAK_KINDS.test(rawKind)) return "Leak";
  switch (rawKind) {
    case "InvalidRead":
    case "InvalidWrite":
    case "InvalidFree":
    case "InvalidJump":
    case "MismatchedFree":
    case "Overlap":
    case "SyscallParam":
    case "UninitCondition":
    case "UninitValue":
      return rawKind;
    default:
      return "Other";
  }
}

function frameOf(node: Element): ValgrindFrame {
  return { file: text(node, "file"), line: number(node, "line") || null, function: text(node, "fn") };
}

/** Parses a Memcheck XML report; a malformed or empty file yields an empty report. */
export function parseValgrindReport(xml: string): ValgrindReport {
  let document: Document;
  try {
    document = new DOMParser().parseFromString(xml, "application/xml");
  } catch {
    return { errors: [], summary: { leakedBytes: 0, errors: 0, kinds: [] } };
  }
  if (document.getElementsByTagName("parsererror").length > 0) {
    return { errors: [], summary: { leakedBytes: 0, errors: 0, kinds: [] } };
  }
  const errors: ValgrindError[] = [];
  for (const error of Array.from(document.getElementsByTagName("error"))) {
    const rawKind = text(error, "kind") ?? "Other";
    const what = text(error, "what") ?? rawKind;
    const stackNode = error.getElementsByTagName("stack")[0];
    const stack = stackNode
      ? Array.from(stackNode.getElementsByTagName("frame")).map(frameOf)
      : Array.from(error.getElementsByTagName("frame")).map(frameOf);
    errors.push({
      kind: classify(rawKind),
      rawKind,
      what,
      stack,
      leakedBytes: number(error, "leakedbytes"),
      primary: stack.find((f) => f.file && f.line) ?? null,
    });
  }
  const counted = new Map<string, number>();
  for (const error of errors) counted.set(error.rawKind, (counted.get(error.rawKind) ?? 0) + 1);
  const kinds = [...counted].map(([kind, count]) => ({ kind, count })).sort((a, b) => b.count - a.count);
  return {
    errors,
    summary: {
      leakedBytes: errors.reduce((sum, e) => sum + e.leakedBytes, 0),
      errors: errors.length,
      kinds,
    },
  };
}

/** How many findings a report has, for "3 errors" wording. */
export function describeValgrindReport(report: ValgrindReport): string {
  const { errors } = report.summary;
  if (errors === 0) return "Valgrind found no errors";
  const leaks = report.errors.filter((e) => e.kind === "Leak").length;
  const other = errors - leaks;
  const parts: string[] = [];
  if (other > 0) parts.push(`${other} error${other === 1 ? "" : "s"}`);
  if (leaks > 0) parts.push(`${leaks} leak record${leaks === 1 ? "" : "s"}`);
  return `Valgrind found ${parts.join(" and ")}`;
}

/**
 * Diagnostics per file: one per finding at its innermost project frame. Frames
 * in the build directory or outside the project produce no diagnostic (the
 * finding still counts in the report).
 */
export function valgrindDiagnostics(report: ValgrindReport, projectRoot: string): Map<string, Diagnostic[]> {
  const byFile = new Map<string, Diagnostic[]>();
  for (const error of report.errors) {
    const frame = error.primary;
    if (!frame?.file || frame.line === null) continue;
    if (!frame.file.startsWith(projectRoot)) continue;
    if (/[\\/](cmake-build-[^\\/]+|[^\\/]*build|out)[\\/]/.test(frame.file.slice(projectRoot.length))) continue;
    const line = Math.max(0, frame.line - 1);
    const diagnostic: Diagnostic = {
      range: { start: { line, col: 0 }, end: { line, col: 0 } },
      severity: error.kind === "Leak" ? "warning" : "error",
      message: frame.function ? tx`${error.what} (in ${frame.function})` : error.what,
      source: "valgrind",
      code: error.rawKind,
      server: null,
    };
    const list = byFile.get(frame.file);
    if (list) list.push(diagnostic);
    else byFile.set(frame.file, [diagnostic]);
  }
  return byFile;
}
