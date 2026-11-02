/**
 * CTest: the console lines it prints (live progress) and the JUnit XML it
 * writes with `--output-junit` (every test, including the passing ones the
 * console only summarises). Covers `add_test`, `gtest_discover_tests`,
 * `catch_discover_tests` and the other CTest-based C++ frameworks.
 */
import type { TestCaseReport } from "@/platform/ipc";
import { parseDuration, type TestEvent, type TestOutputParser } from "./types";

/** `1/3 Test #1: passing ..........................   Passed    0.01 sec` */
const LINE = /^\s*\d+\/\d+\s+Test\s+#\d+:\s+(.*?)\s+\.{2,}\s*(.*?)\s*$/;

function statusOf(text: string): "passed" | "failed" | "skipped" {
  if (/^passed/i.test(text)) return "passed";
  if (/not run|disabled|skipped/i.test(text)) return "skipped";
  return "failed";
}

/** Live progress from CTest's own output; the report below supplies the rest. */
export function ctestParser(): TestOutputParser {
  return {
    feed(line) {
      const m = LINE.exec(line);
      if (!m) return [];
      const name = m[1]!.trim();
      // `***Failed    0.01 sec` — the asterisks mark a non-passing result.
      const rest = m[2]!.replace(/\*/g, " ").trim();
      const split = rest.search(/\s{2,}/);
      const status = statusOf(split < 0 ? rest : rest.slice(0, split));
      const duration = parseDuration(split < 0 ? "" : rest.slice(split));
      return [{ type: "result", path: [name], status, durationMs: duration }];
    },
    end: () => [],
  };
}

/** A flat node per test: CTest report class names repeat the test name. */
export function ctestReportEvents(reports: readonly TestCaseReport[]): TestEvent[] {
  const events: TestEvent[] = [];
  for (const r of reports) {
    const name = r.name || r.className;
    if (!name) continue;
    const path = [name];
    events.push({ type: "result", path, status: r.status, durationMs: r.durationMs ?? undefined });
    if (r.status === "failed") {
      const text = [r.message, r.output].filter(Boolean).join("\n");
      for (const line of text.split("\n").slice(0, 200)) events.push({ type: "output", path, line });
    }
  }
  return events;
}
