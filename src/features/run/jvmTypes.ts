/**
 * Maven / Gradle configurations hand their VM options to the JVM the build forks
 * (`spring-boot:run`, tests, `bootRun`) rather than to the build tool's own JVM.
 */
import { call, commands, type RunConfig } from "@/platform/ipc";
import type { RunConfigTypeField } from "@/workbench/contributions";
import { parseArgs } from "./configs";

export const isWindows = () => typeof navigator !== "undefined" && /win/i.test(navigator.platform);

/** VM options / program arguments as typed; on Windows a backslash is a path separator. */
export const splitJvmArgs = (text: string | undefined, windows = isWindows()) => parseArgs(text ?? "", windows);

const VM_OPTIONS_PLACEHOLDER = "e.g. -Xmx1g -Dkey=value --add-opens java.base/java.lang=ALL-UNNAMED";

const vmOptionsField = (hint?: string, visible?: RunConfigTypeField["visible"]): RunConfigTypeField => ({
  key: "vmOptions",
  label: "VM options",
  control: "textarea",
  rows: 2,
  mono: true,
  placeholder: VM_OPTIONS_PLACEHOLDER,
  ...(hint ? { hint } : {}),
  ...(visible ? { visible } : {}),
});

const profilesField = (hint: string, visible?: RunConfigTypeField["visible"]): RunConfigTypeField => ({
  key: "activeProfiles",
  label: "Active profiles",
  mono: true,
  placeholder: "e.g. dev,local",
  hint,
  ...(visible ? { visible } : {}),
});

/**
 * One value for a Maven property holding JVM arguments (`spring-boot.run.jvmArguments`,
 * `argLine`): the plugins split it on whitespace, honouring "…" and '…'.
 */
export function joinPropertyArgs(args: readonly string[]): string {
  return args
    .map((a) => {
      if (a && !/[\s"']/.test(a)) return a;
      return a.includes('"') ? `'${a}'` : `"${a}"`;
    })
    .join(" ");
}

const words = (tasks: string | undefined) => (tasks ?? "").split(/\s+/).filter(Boolean);

const isSpringBootRun = (goal: string) =>
  goal === "spring-boot:run" || /:spring-boot-maven-plugin:[^:]*:run$/.test(goal);
const isMavenTestGoal = (goal: string) =>
  /^(test|verify|integration-test|surefire:test|failsafe:integration-test)$/.test(goal);
const isBootRun = (task: string) => /(^|:)bootRun$/.test(task);
const isGradleJvmTask = (task: string) => /(^|:)(test|bootRun|run|integrationTest|check)$/.test(task);

/** What a build tool configuration does with VM options and Spring Boot profiles. */
export interface ForkedJvm {
  fields: readonly RunConfigTypeField[];
  /** Arguments appended to the build tool's command line. */
  args(params: Readonly<Record<string, string>>): string[];
  prepareLaunch?(config: RunConfig): Promise<RunConfig>;
}

export const mavenForkedJvm: ForkedJvm = {
  fields: [
    vmOptionsField("Passed to the JVM that spring-boot:run or the tests start, not to Maven", (p) =>
      words(p.tasks).some((g) => isSpringBootRun(g) || isMavenTestGoal(g)),
    ),
    profilesField("Comma separated; passed as -Dspring-boot.run.profiles", (p) => words(p.tasks).some(isSpringBootRun)),
  ],
  args(p) {
    const goals = words(p.tasks);
    const vm = splitJvmArgs(p.vmOptions);
    const out: string[] = [];
    if (goals.some(isSpringBootRun)) {
      if (vm.length) out.push(`-Dspring-boot.run.jvmArguments=${joinPropertyArgs(vm)}`);
      const profiles = p.activeProfiles?.trim();
      if (profiles) out.push(`-Dspring-boot.run.profiles=${profiles}`);
    } else if (vm.length && goals.some(isMavenTestGoal)) {
      out.push(`-DargLine=${joinPropertyArgs(vm)}`);
    }
    return out;
  },
};

/** VM options (and profiles, as `-Dspring.profiles.active`) of a Gradle configuration's forked JVMs. */
export function gradleJvmArgs(p: Readonly<Record<string, string>>): string[] {
  const tasks = words(p.tasks);
  if (!tasks.some(isGradleJvmTask)) return [];
  const profiles = tasks.some(isBootRun) ? p.activeProfiles?.trim() : "";
  return [...splitJvmArgs(p.vmOptions), ...(profiles ? [`-Dspring.profiles.active=${profiles}`] : [])];
}

export const gradleForkedJvm: ForkedJvm = {
  fields: [
    vmOptionsField("Passed to the JVMs the tasks start (bootRun, run, test), not to Gradle", (p) =>
      words(p.tasks).some(isGradleJvmTask),
    ),
    profilesField("Comma separated; passed as -Dspring.profiles.active", (p) => words(p.tasks).some(isBootRun)),
  ],
  // Gradle has no command-line switch for them: an init script adds them to the JavaExec / Test tasks.
  args: () => [],
  async prepareLaunch(config) {
    const jvmArgs = gradleJvmArgs(config.params ?? {});
    if (!jvmArgs.length) return config;
    if (config.target) throw new Error("VM options of Gradle configurations are not applied on run targets");
    const script = await call(commands.runGradleJvmArgsScript(jvmArgs));
    return { ...config, args: ["--init-script", script, ...config.args] };
  },
};
