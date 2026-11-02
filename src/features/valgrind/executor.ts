/**
 * "Run with Valgrind Memcheck": the same program under `valgrind --xml=yes`,
 * then the XML report becomes diagnostics at the innermost project frame of
 * every finding (CLion's Valgrind integration). The run itself stays a normal
 * Run tab, so the program's console output and the Valgrind messages are both
 * there; the report path is a temporary directory, so nothing lands in the
 * project.
 */
import { executeAction, registerActions } from "@/platform/actions";
import { call, commands, isTauri, type RunConfig } from "@/platform/ipc";
import { tx } from "@/platform/i18n";
import { publishLocalDiagnostics } from "@/platform/localDiagnostics";
import { useProject } from "@/platform/project";
import {
  notifications,
  toolWindowService,
  type ExecutorLaunch,
  type RunExecutorContribution,
} from "@/workbench/contributions";
import { describeValgrindReport, parseValgrindReport, valgrindDiagnostics } from "./report";

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** Binaries Valgrind can run: native targets and single compiled files. */
export function valgrindSupports(config: RunConfig): boolean {
  return (config.kind === "native" || config.kind === "cc") && !!config.command;
}

/** One run's report: a fresh directory keeps concurrent runs apart. */
async function reportPath(name: string): Promise<string> {
  const { tempDir } = await import("@tauri-apps/api/path");
  const base = (await tempDir()).replace(/[\\/]+$/, "");
  const dir = `${base}/openclion-valgrind/${Date.now()}`;
  await call(commands.fsCreateDir(dir));
  const safe = name.replace(/[^\w.-]+/g, "-");
  return `${dir}/${safe}.xml`;
}

async function prepare(config: RunConfig): Promise<ExecutorLaunch | null> {
  if (!isTauri()) {
    notifications().notify({ group: "run", level: "warning", title: "Valgrind needs the desktop app" });
    return null;
  }
  const root = useProject.getState().project?.root ?? "";
  let report: string;
  try {
    report = await reportPath(config.name);
  } catch (e) {
    notifications().notify({
      group: "run",
      level: "error",
      key: "valgrind-report",
      title: `Cannot run '${config.name}' with Valgrind`,
      body: errorText(e),
    });
    return null;
  }
  const launch: RunConfig = {
    ...config,
    id: `${config.id}#valgrind`,
    name: `Valgrind: ${config.name}`,
    command: "valgrind",
    // `--error-exitcode` keeps the run "succeeded": the findings are the result,
    // and a failing exit code would make every leak look like a broken run.
    args: [
      "--tool=memcheck",
      "--leak-check=full",
      "--num-callers=40",
      "--xml=yes",
      `--xml-file=${report}`,
      config.command,
      ...config.args,
    ],
    debugAdapter: null,
    isTest: false,
  };
  return {
    launch,
    finished: ({ stopped }) => {
      if (stopped) return;
      void collect(report, root, config.name);
    },
  };
}

/** Reads the report of a finished run and publishes what it found. */
async function collect(report: string, root: string, name: string) {
  let xml: string;
  try {
    xml = await call(commands.fsReadText(report));
  } catch {
    // Valgrind missing or killed before writing: the console already says why.
    notifications().notify({
      group: "run",
      level: "warning",
      key: "valgrind-empty",
      title: `No Valgrind report for '${name}'`,
      body: "The run did not produce a report. Is valgrind installed and on PATH?",
    });
    return;
  }
  const parsed = parseValgrindReport(xml);
  for (const [file, diagnostics] of valgrindDiagnostics(parsed, root)) {
    publishLocalDiagnostics(file, "valgrind", diagnostics);
  }
  const found = parsed.summary.errors > 0;
  notifications().notify({
    group: "run",
    level: found ? "warning" : "info",
    key: "valgrind-done",
    title: `${name}: ${describeValgrindReport(parsed)}`,
    body: found ? tx`Open Problems to see where the findings are.` : undefined,
    ...(found
      ? {
          actions: [
            {
              label: "Show Problems",
              run: () => toolWindowService().show("problems", true),
            },
          ],
        }
      : {}),
  });
}

export const valgrindExecutor: RunExecutorContribution = {
  id: "valgrind",
  label: (name) => `Run '${name}' with Valgrind`,
  icon: "toolwindow-profiler",
  action: "run.valgrind",
  order: 30,
  supports: valgrindSupports,
  prepare,
};

/** Run › Run with Valgrind Memcheck. */
export function registerValgrindActions(): () => void {
  return registerActions([
    {
      id: "run.valgrind",
      title: "Run with Valgrind Memcheck",
      description: "Runs the selected configuration under Valgrind and reports its findings",
      category: "Run",
      icon: "toolwindow-profiler",
      when: "projectOpen",
      keywords: ["valgrind", "memcheck", "memory", "leak"],
      run: ({ arg }) => {
        void executeAction("run.withExecutor", { executor: "valgrind", arg });
      },
    },
  ]);
}
