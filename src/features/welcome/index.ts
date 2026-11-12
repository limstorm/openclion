/** Welcome screen, recent projects (File › Open Recent) and reopening the last project on startup. */
import { registerActions } from "@/platform/actions";
import { holdSplash, LAUNCH_WAIT_MS } from "@/platform/splash";
import { provideProjectPopup, provideWelcomeView, registerOverlay } from "@/workbench/contributions";
import { ProjectPopup } from "./ProjectPopup";
import { RecentProjectsPopup, showRecentProjectsPopup } from "./RecentProjectsPopup";
import { reopenLastProject, trackSession } from "./session";
import { WelcomeView } from "./WelcomeView";

export function activate(): () => void {
  const disposers = [
    trackSession(),
    provideWelcomeView(WelcomeView),
    provideProjectPopup(ProjectPopup),
    registerOverlay({ id: "welcome.recentProjects", component: RecentProjectsPopup }),
    registerActions([
      {
        id: "welcome.recentProjects",
        title: "Recent Projects…",
        category: "File",
        icon: "history",
        keywords: ["open recent", "projects"],
        run: showRecentProjectsPopup,
      },
      {
        id: "file.openRecent",
        title: "Open Recent…",
        category: "File",
        icon: "history",
        keywords: ["recent projects", "reopen"],
        run: showRecentProjectsPopup,
      },
    ]),
  ];
  // Keep the splash up for the restore (at most the launch budget); a fast reopen never
  // shows the welcome screen at all, a slow one continues behind the workbench.
  const reopening = reopenLastProject();
  void reopening;
  holdSplash(reopening, LAUNCH_WAIT_MS);
  return () => disposers.reverse().forEach((d) => d());
}
