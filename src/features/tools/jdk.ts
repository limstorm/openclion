/**
 * `java.downloadJdk` (arg: `DownloadJdkArgs`): the one-click fix offered where a
 * run configuration or a Maven / Gradle sync lacks a new enough JDK. Downloads
 * Eclipse Temurin N into the IDE's JDK folder (found by JDK detection from then
 * on); progress and Cancel live in the status bar's background tasks.
 */
import type { Sdk } from "@/bindings";
import { executeAction, registerActions } from "@/platform/actions";
import { isCancelled, isTauri } from "@/platform/ipc";
import {
  DOWNLOAD_JDK_ACTION,
  downloadJdk,
  isDownloadJdkArgs,
  TEMURIN,
  type DownloadJdkArgs,
  type JdkDownload,
} from "@/platform/jdk";
import { notifications } from "@/workbench/contributions";
import { queueTask, upsertTask } from "@/workbench/tasks";
import { DOWNLOAD_QUEUE } from "./store";
import { tx } from "@/platform/i18n";

const JAVA_SETTINGS = "languages.java";
const running = new Map<number, Promise<Sdk | null>>();

export function downloadJdkFor(args: DownloadJdkArgs): Promise<Sdk | null> {
  const existing = running.get(args.version);
  if (existing) return existing;
  const title = `Downloading JDK ${args.version}`;
  const taskId = `jdk-download:${args.version}`;
  const key = `jdk-download-${args.version}`;
  const p = (async () => {
    try {
      // Waits for the downloads ahead of it; cancelling it there means it never starts.
      const sdk = await queueTask(DOWNLOAD_QUEUE, { id: taskId, title }, () => {
        const cancel = () => job.cancel();
        upsertTask({ id: taskId, title, message: "Eclipse Temurin", fraction: null, done: false, cancel });
        const job: JdkDownload = downloadJdk(TEMURIN, args.version, null, (e) => {
          const message = e.type === "line" ? e.text : e.message;
          const fraction = e.type === "progress" ? e.fraction : null;
          upsertTask({ id: taskId, title, message, fraction, done: false, cancel });
        });
        return job.promise;
      });
      notifications().notify({
        group: "tools",
        level: "info",
        key,
        title: `${sdk.name} downloaded`,
        body: tx`Installed in ${sdk.path}; it is used for projects targeting Java ${args.version}.`,
      });
      return sdk;
    } catch (e) {
      if (isCancelled(e)) return null;
      notifications().notify({
        group: "tools",
        level: "error",
        key,
        title: `Failed to download JDK ${args.version}`,
        body: e instanceof Error ? e.message : String(e),
        actions: [{ label: "Java Settings…", run: () => void executeAction("file.settings", JAVA_SETTINGS) }],
      });
      return null;
    } finally {
      upsertTask({ id: taskId, title, message: null, fraction: 1, done: true });
      running.delete(args.version);
    }
  })();
  running.set(args.version, p);
  return p;
}

export function registerJdkActions(): () => void {
  return registerActions([
    {
      id: DOWNLOAD_JDK_ACTION,
      title: "Download JDK…",
      category: "Java",
      hidden: true,
      // Without a version: the Java settings, whose JDK pickers offer Download JDK….
      run: async ({ arg }) => {
        if (isDownloadJdkArgs(arg) && isTauri()) await downloadJdkFor(arg);
        else await executeAction("file.settings", JAVA_SETTINGS);
      },
    },
  ]);
}
