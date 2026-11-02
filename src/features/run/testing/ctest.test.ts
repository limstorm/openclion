import { describe, expect, it } from "vitest";
import type { RunConfig } from "@/platform/ipc";
import { ctestParser, ctestReportEvents } from "./ctest";
import { testsConfig } from "./rerun";
import { frameworkFor } from "./index";

const feed = (lines: string[]) => {
  const parser = ctestParser();
  return [...lines.flatMap((line) => parser.feed(line)), ...(parser.end?.() ?? [])];
};

describe("CTest console", () => {
  it("turns the per-test progress lines into results", () => {
    const events = feed([
      "Test project /tmp/demo/build",
      "    Start 1: passing",
      "1/3 Test #1: passing ..........................   Passed    0.01 sec",
      "2/3 Test #2: failing ..........................***Failed    0.02 sec",
      "3/3 Test #3: skipped ..........................***Not Run (Disabled)   0.00 sec",
      "50% tests passed, 1 tests failed out of 2",
    ]);
    expect(events).toEqual([
      { type: "result", path: ["passing"], status: "passed", durationMs: 10 },
      { type: "result", path: ["failing"], status: "failed", durationMs: 20 },
      { type: "result", path: ["skipped"], status: "skipped", durationMs: 0 },
    ]);
  });

  it("ignores unrelated output", () => {
    expect(feed(["Start 1: x", "Errors while running CTest", ""])).toEqual([]);
  });
});

describe("CTest JUnit report", () => {
  it("maps every case, including passes, onto a flat tree", () => {
    const events = ctestReportEvents([
      { className: "passing", name: "passing", status: "passed", durationMs: 6.6, message: null, output: null },
      {
        className: "failing",
        name: "failing",
        status: "failed",
        durationMs: 5.7,
        message: "Failed",
        output: "some output",
      },
      { className: "skipped", name: "skipped", status: "skipped", durationMs: 0, message: null, output: "Disabled" },
    ]);
    expect(events[0]).toEqual({ type: "result", path: ["passing"], status: "passed", durationMs: 6.6 });
    expect(events[1]).toEqual({ type: "result", path: ["failing"], status: "failed", durationMs: 5.7 });
    // Failure message and captured output become the node's console lines.
    expect(events.slice(2, 4)).toEqual([
      { type: "output", path: ["failing"], line: "Failed" },
      { type: "output", path: ["failing"], line: "some output" },
    ]);
    expect(events.at(-1)).toEqual({ type: "result", path: ["skipped"], status: "skipped", durationMs: 0 });
  });
});

describe("CTest integration", () => {
  it("recognizes the CTest configuration", () => {
    expect(frameworkFor({ kind: "cmake", command: "ctest", args: ["--test-dir", "build"] })).toBe("ctest");
    expect(frameworkFor({ kind: "cmake", command: "cmake", args: ["--build", "build"] })).toBe("auto");
  });

  it("reruns only the selected tests with -R, keeping the report arguments", () => {
    const config: RunConfig = {
      id: "cmake:build:test",
      name: "CTest (build)",
      kind: "cmake",
      command: "ctest",
      args: [
        "--test-dir",
        "build",
        "-C",
        "Debug",
        "--output-junit",
        "build/ctest-junit.xml",
        "--output-on-failure",
        "-R",
        "old",
      ],
      cwd: null,
      env: {},
      detected: true,
      isTest: true,
      debugAdapter: null,
      params: { ctestReport: "build/ctest-junit.xml" },
    };
    const rerun = testsConfig(config, [["greeter"], ["math.adds"]], "Failed: CTest (build)");
    expect(rerun?.args).toEqual([
      "--test-dir",
      "build",
      "-C",
      "Debug",
      "--output-junit",
      "build/ctest-junit.xml",
      "--output-on-failure",
      "-R",
      "^(greeter|math\\.adds)$",
    ]);
    expect(rerun?.params?.ctestReport).toBe("build/ctest-junit.xml");
  });
});
