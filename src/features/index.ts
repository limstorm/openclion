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
import { activate as valgrind } from "./valgrind";
import { activate as sanitizers } from "./sanitizers";
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
import { activate as viewers } from "./viewers";
import { activate as web } from "./web";
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
  ["valgrind", valgrind],
  ["sanitizers", sanitizers],
  ["build", build],
  ["cmake", cmake],
  ["templates", templates],
  ["todo", todo],
  ["settings", settings],
  ["perf", perf],
  ["scratch", scratch],
  ["viewers", viewers],
  ["web", web],
  ["analysis", analysis],
  ["services", services],
  ["agents", agents],
  ["plugins", plugins],
  ["tools", tools],
  ["vim", vim],
  ["ssh", ssh],
  ["remote-dev", remoteDev],
];

/**
 * Tool-window and settings features that the first paint does not need: their
 * modules load after the workbench is on screen (`import()` in dev keeps them
 * out of the blocking module graph, which cost ~2 s of start-up there).
 */
const deferredFeatures: Array<[string, () => Promise<{ activate: () => void | (() => void) }>]> = [
  ["database", () => import("./database")],
  ["http-client", () => import("./http-client")],
  ["notebook", () => import("./notebook")],
  ["github", () => import("./github")],
  ["coverage", () => import("./coverage")],
  ["profiler", () => import("./profiler")],
  ["deployment", () => import("./deployment")],
  ["libraries", () => import("./libraries")],
];

export function activateFeatures(): () => void {
  const disposers: Array<() => void> = [];
  for (const [name, activate] of features) {
    try {
      const d = activate();
      if (d) disposers.push(d);
    } catch (e) {
      console.error(`feature ${name} failed to activate`, e);
    }
  }
  const deferred: Array<() => void> = [];
  // After the first frame: the workbench is interactive before these register.
  requestAnimationFrame(() => {
    void (async () => {
      for (const [name, load] of deferredFeatures) {
        try {
          const d = (await load()).activate();
          if (d) deferred.push(d);
        } catch (e) {
          console.error(`feature ${name} failed to activate`, e);
        }
      }
    })();
  });
  return () => {
    deferred.reverse().forEach((d) => d());
    disposers.reverse().forEach((d) => d());
  };
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
