/**
 * "Run with sanitizers": the binary is already built with `-fsanitize=…` (see
 * the CMake profile's sanitizer toggle); this executor makes the run report
 * readable — symbolized stacks and leak detection through the sanitizer
 * options — and turns the reports the program prints on stderr into Problems
 * entries at the failing source line.
 */
import { executeAction, registerActions } from "@/platform/actions";
import type { RunConfig } from "@/platform/ipc";
import { tx } from "@/platform/i18n";
import { publishLocalDiagnostics } from "@/platform/localDiagnostics";
import { useProject } from "@/platform/project";
import { runtime, useRun } from "@/features/run/store";
import {
  notifications,
  toolWindowService,
  type ExecutorLaunch,
  type RunExecutorContribution,
} from "@/workbench/contributions";
import { describeSanitizerFindings, parseSanitizerOutput, sanitizerDiagnostics } from "./report";

/** Sanitizer runtime options: symbolized stacks, leaks on, no abort on the first error. */
export const SANITIZER_ENV: Readonly<Record<string, string>> = {
  ASAN_OPTIONS: "symbolize=1:detect_leaks=1:abort_on_error=0:print_summary=1",
  LSAN_OPTIONS: "symbolize=1",
  UBSAN_OPTIONS: "print_stacktrace=1",
  TSAN_OPTIONS: "halt_on_error=0:second_deadlock_stack=1",
};

/** The configuration's environment with the sanitizer options applied. */
export function sanitizerEnv(env: Readonly<Record<string, string>>): Record<string, string> {
  return { ...SANITIZER_ENV, ...env };
}

/** Binaries that can carry a sanitizer runtime. */
export function sanitizerSupports(config: RunConfig): boolean {
  return (config.kind === "native" || config.kind === "cc") && !!config.command;
}

async function prepare(config: RunConfig): Promise<ExecutorLaunch | null> {
  const launchStarted = Date.now();
  const launch: RunConfig = {
    ...config,
    id: `${config.id}#sanitizer`,
    name: `Sanitizers: ${config.name}`,
    env: sanitizerEnv(config.env),
    debugAdapter: null,
    isTest: false,
  };
  return {
    launch,
    finished: ({ stopped }) => {
      if (stopped) return;
      void collectSanitizerReport(config, launchStarted);
    },
  };
}

/**
 * Reads the finished run's console. The session keeps the *original*
 * configuration, so the newest session of `config` started after the launch is
 * the one this executor produced.
 */
async function collectSanitizerReport(config: RunConfig, launchedAt: number) {
  const session = useRun
    .getState()
    .sessions.filter((s) => s.config.id === config.id && s.startedAt >= launchedAt)
    .sort((a, b) => b.startedAt - a.startedAt)[0];
  const lines = session ? runtime(session.id)?.console.lines : undefined;
  const text = lines?.map((l) => l.text).join("\n") ?? "";
  const findings = parseSanitizerOutput(text);
  const root = useProject.getState().project?.root ?? "";
  for (const [file, diagnostics] of sanitizerDiagnostics(findings, root)) {
    publishLocalDiagnostics(file, "sanitizer", diagnostics);
  }
  const found = findings.length > 0;
  notifications().notify({
    group: "run",
    level: found ? "error" : "info",
    key: "sanitizer-done",
    title: `${config.name}: ${describeSanitizerFindings(findings)}`,
    body: found
      ? tx`Open Problems to see the failing lines.`
      : tx`Nothing was reported. Was the program built with -fsanitize=address,undefined?`,
    ...(found
      ? {
          actions: [{ label: "Show Problems", run: () => toolWindowService().show("problems", true) }],
        }
      : {}),
  });
}

export const sanitizerExecutor: RunExecutorContribution = {
  id: "sanitizers",
  label: (name) => `Run '${name}' with Sanitizers`,
  icon: "run-coverage",
  action: "run.sanitizers",
  order: 31,
  supports: sanitizerSupports,
  prepare,
};

/** Run › Run with Sanitizers. */
export function registerSanitizerActions(): () => void {
  return registerActions([
    {
      id: "run.sanitizers",
      title: "Run with Sanitizers",
      description: "Runs the selected configuration with ASan/UBSan options and reports what it finds",
      category: "Run",
      icon: "run-coverage",
      when: "projectOpen",
      keywords: ["asan", "ubsan", "tsan", "sanitizer", "address", "leak"],
      run: ({ arg }) => {
        void executeAction("run.withExecutor", { executor: "sanitizers", arg });
      },
    },
  ]);
}
