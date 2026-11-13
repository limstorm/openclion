/**
 * Feature activation. Each feature exports `activate(): void | (() => void)`
 * that registers actions, tool windows, status items and services, returning
 * a disposer. The editor provides the editor service; app must install its launch
 * barrier before welcome can restore a remembered project.
 */
import { activate as projectTrust } from "./project-trust";
import { activate as editor } from "@/editor/core";
import { activate as editorLsp } from "@/editor/lsp";
import { activate as welcome } from "./welcome";
import { activate as projectTree } from "./project-tree";
import { activate as search } from "./search";
import { activate as dependencies } from "./dependencies";
import { activate as codeInsight } from "./code-insight";
import { activate as vcs, activateSettingsPages as vcsPages } from "./vcs";
import { activate as terminal } from "./terminal";
import { activate as run } from "./run";
import { activate as runAnything } from "./run-anything";
import { activate as macros } from "./macros";
import { activate as debuggerFeature } from "./debugger";
import { activate as build } from "./build";
import { activate as cmake } from "./cmake";
import { activate as templates, activateSettingsPages as templatesPages } from "./templates";
import { activate as todo, activateSettingsPages as todoPages } from "./todo";
import { activate as settings, activateSettingsWindow } from "./settings";
import { activate as app, activateLocale as appLocale } from "./app";
import { activate as perf } from "./perf";
import { activate as plugins, activateSettingsPages as pluginsPages } from "./plugins";
import { activate as tools, activateSettingsPages as toolsPages } from "./tools";
import { activate as scratch } from "./scratch";
import { activate as libraries } from "./libraries";
import { activate as viewers } from "./viewers";
import { activate as analysis } from "./analysis";
import { activate as services } from "./services";
import { activate as agents, activateSettingsPages as agentsPages } from "./agents";
import { activate as vim, activateSettingsPages as vimPages } from "./vim";
import { activate as ssh } from "./ssh";
import { activate as remoteDev } from "./remote-dev";

type Activate = () => void | (() => void);

const features: Array<[string, Activate]> = [
  ["project-trust", projectTrust],
  ["editor", editor],
  ["editor-lsp", editorLsp],
  ["app", app],
  ["welcome", welcome],
  ["project-tree", projectTree],
  ["search", search],
  ["code-insight", codeInsight],
  ["dependencies", dependencies],
  ["vcs", vcs],
  ["terminal", terminal],
  ["run", run],
  ["run-anything", runAnything],
  ["macros", macros],
  ["debugger", debuggerFeature],
  ["build", build],
  ["cmake", cmake],
  ["templates", templates],
  ["todo", todo],
  ["settings", settings],
  ["perf", perf],
  ["plugins", plugins],
  ["tools", tools],
  ["scratch", scratch],
  ["libraries", libraries],
  ["viewers", viewers],
  ["analysis", analysis],
  ["services", services],
  ["agents", agents],
  ["vim", vim],
  ["ssh", ssh],
  ["remote-dev", remoteDev],
];

/**
 * Features nothing in the start-up UI needs: no start-up view, toolbar, status item or
 * keybinding of theirs is on screen before the first idle moment. Loading them on the
 * first idle callback keeps ~1 MB of their source out of the chunk the first paint
 * waits for; their actions and tool windows appear a few hundred milliseconds later,
 * together with everything else the workbench registers.
 */
const deferredFeatures: Record<string, () => Promise<{ activate: Activate }>> = {
  coverage: () => import("./coverage"),
  profiler: () => import("./profiler"),
  valgrind: () => import("./valgrind"),
  sanitizers: () => import("./sanitizers"),
  notebook: () => import("./notebook"),
  web: () => import("./web"),
  github: () => import("./github"),
  "http-client": () => import("./http-client"),
  database: () => import("./database"),
  deployment: () => import("./deployment"),
};

export interface ActivateFeaturesOptions {
  /** Load [`deferredFeatures`] once the browser is idle instead of now. The app passes
   *  this; tests that assert on those features' actions activate them synchronously. */
  deferOptional?: boolean;
}

export function activateFeatures(options: ActivateFeaturesOptions = {}): () => void {
  const disposers: Array<() => void> = [];
  const run = (name: string, activate: Activate) => {
    try {
      const d = activate();
      if (d) disposers.push(d);
    } catch (e) {
      console.error(`feature ${name} failed to activate`, e);
    }
  };
  for (const [name, activate] of features) run(name, activate);
  if (options.deferOptional) {
    let cancelled = false;
    const load = () => {
      if (cancelled) return;
      const pending = Object.entries(deferredFeatures);
      const next = () => {
        const entry = pending.shift();
        if (cancelled || !entry) return;
        const [name, loadModule] = entry;
        void loadModule()
          .then((m) => {
            if (!cancelled) run(name, m.activate);
          })
          .catch((e: unknown) => console.error(`feature ${name} failed to load`, e))
          .finally(() => idle(next));
      };
      next();
    };
    idle(load);
    return () => {
      cancelled = true;
      disposers.reverse().forEach((d) => d());
    };
  }
  return () => disposers.reverse().forEach((d) => d());
}

/** Runs `f` when the browser is next idle, with a bounded wait. */
function idle(f: () => void): void {
  if (typeof requestIdleCallback === "function") requestIdleCallback(f, { timeout: 2000 });
  else setTimeout(f, 32);
}

export { SettingsWindow, settingsWindowParent } from "./settings";

/**
 * The native Settings window of project window `parent`: only the settings
 * pages (no workbench, project, editors or background services).
 */
export function activateSettingsWindowFeatures(parent: string): () => void {
  const pages: Array<[string, Activate]> = [
    ["project-trust", projectTrust],
    ["locale", appLocale],
    ["settings", () => activateSettingsWindow(parent)],
    ["templates", templatesPages],
    ["todo", todoPages],
    ["plugins", pluginsPages],
    ["tools", toolsPages],
    ["agents", agentsPages],
    ["vim", vimPages],
    ["vcs", vcsPages],
  ];
  const disposers: Array<() => void> = [];
  for (const [name, activate] of pages) {
    try {
      const d = activate();
      if (d) disposers.push(d);
    } catch (e) {
      console.error(`settings pages of ${name} failed to activate`, e);
    }
  }
  return () => disposers.reverse().forEach((d) => d());
}
