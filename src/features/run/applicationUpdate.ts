import { create } from "zustand";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { executeAction } from "@/platform/actions";
import { call, commands, isTauri, type RunConfig } from "@/platform/ipc";
import {
  maybeEditorService,
  notifications,
  useDebugRunSessions,
  type DebugServiceSession,
} from "@/workbench/contributions";
import { showContextMenu } from "@/ui";
import { buildTomcatArtifact } from "./tomcat";
import { startRun, stopSession, useRun, waitForExit, type RunSession } from "./store";
import { isUpdatable, updateAllowed } from "./updatePolicies";

export type UpdateSession = Pick<RunSession, "id" | "config"> & { debug?: DebugServiceSession };
export const useApplicationUpdates = create<{ ids: ReadonlySet<string> }>(() => ({ ids: new Set() }));
const updatingConfigs = new Set<string>();

export function updatableSessions(): UpdateSession[] {
  const runs = useRun.getState().sessions.filter((s) => isUpdatable(s.config) && s.status === "running");
  const debug = useDebugRunSessions
    .getState()
    .serviceSessions.filter(
      (s) =>
        isUpdatable(s.config) &&
        (s.status === "running" || s.status === "paused") &&
        !runs.some((r) => r.runId === s.processId && r.config.id === s.config.id),
    )
    .map((s) => ({ id: `debug:${s.id}`, config: s.config, debug: s }));
  return [...runs, ...debug];
}

function params(session: UpdateSession): RunConfig["params"] {
  return useRun.getState().configs.find((c) => c.id === session.config.id)?.params ?? session.config.params;
}

export async function updateApplication(session: UpdateSession, mode: string): Promise<void> {
  if (!updateAllowed(session.config, mode) || updatingConfigs.has(session.config.id)) return;
  updatingConfigs.add(session.config.id);
  useApplicationUpdates.setState((s) => ({ ids: new Set([...s.ids, session.id]) }));
  try {
    const debugging =
      !!session.debug || useDebugRunSessions.getState().sessions.some((s) => s.configId === session.config.id);
    if (mode === "restart") {
      if (session.debug) {
        await session.debug.stop();
        await executeAction("run.debugConfig", session.config);
      } else if (debugging) {
        await stopSession(session.id);
        await waitForExit([session.id]);
        await executeAction("run.debugConfig", session.config);
      } else await startRun(session.config, session.id);
      return;
    }
    await maybeEditorService()?.saveAll();
    await buildTomcatArtifact(session.config);
    await call(commands.runTomcatUpdate(session.config, mode));
    // Classes need a fresh class loader: redeploy after copying them.
    if (mode === "classes") await call(commands.runTomcatUpdate(session.config, "redeploy"));
    notifications().notify({ group: "run", level: "info", title: "Application updated" });
    if (!session.debug) useRun.setState({ activeSession: session.id });
  } catch (e) {
    notifications().notify({ group: "run", level: "error", title: "Application update failed", body: String(e) });
  } finally {
    updatingConfigs.delete(session.config.id);
    useApplicationUpdates.setState((s) => ({ ids: new Set([...s.ids].filter((id) => id !== session.id)) }));
  }
}

export function showApplicationUpdate(sessionId?: string): void {
  const state = useRun.getState();
  const candidates = updatableSessions();
  const session = candidates.find((s) => s.id === (sessionId || state.activeSession)) ?? candidates[0];
  if (!session) return;
  const policy = params(session)?.onUpdate ?? "ask";
  if (policy !== "ask" && updateAllowed(session.config, policy)) {
    void updateApplication(session, policy);
    return;
  }
  const choices = [
    ["resources", "Update resources"],
    ["classes", "Update classes and resources"],
    ["redeploy", "Redeploy"],
    ["restart", "Restart server"],
  ];
  showContextMenu({ x: window.innerWidth / 2 - 140, y: 100 }, [
    { kind: "header", label: "Update Application" },
    ...choices.map(([mode, label]) => ({
      label: label!,
      disabled: useApplicationUpdates.getState().ids.has(session.id) || !updateAllowed(session.config, mode!),
      run: () => {
        void updateApplication(session, mode!);
      },
    })),
  ]);
}

export function updateOnDeactivation(): void {
  for (const session of updatableSessions()) {
    const mode = params(session)?.onDeactivation;
    if (mode === "resources" || mode === "classes") void updateApplication(session, mode);
  }
}

export function listenForApplicationDeactivation(): () => void {
  if (!isTauri()) return () => {};
  const listener = getCurrentWebviewWindow().listen("ide://application-deactivated", updateOnDeactivation);
  return () => {
    void listener.then((unlisten) => unlisten());
  };
}
