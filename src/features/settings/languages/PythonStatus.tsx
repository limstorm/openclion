/**
 * Python interpreter support: path detection and the interpreter actions.
 *
 * PyCharm's status-bar widget (a "Python Interpreter: …" button in the right-hand
 * group) is gone: this is a C/C++ IDE, and that slot now shows the Git branch
 * (`features/vcs/index.ts`). The actions stay, so Settings › Python and
 * "Select Python Interpreter" keep working.
 */
import { executeAction, registerActions } from "@/platform/actions";
import { call, commands } from "@/platform/ipc";
import { useProject } from "@/platform/project";
import { notifications } from "@/workbench/contributions";
import type { Sdk } from "./api";

const markers = [
  "pyproject.toml",
  "setup.py",
  "setup.cfg",
  "requirements.txt",
  "Pipfile",
  "pyrightconfig.json",
  "ty.toml",
  "uv.lock",
  "poetry.lock",
  "SConstruct",
  "SConscript",
  ".python-version",
];

export function isPythonPath(path: string): boolean {
  const name = path.replaceAll("\\", "/").split("/").at(-1)!;
  return /\.(py|pyi|pyw)$/.test(name) || markers.includes(name) || /^requirements[\w.-]*\.txt$/.test(name);
}

export function interpreterChoices(sdks: Sdk[], current: string | null): { path: string; name: string }[] {
  const choices = [...new Map(sdks.map((sdk) => [sdk.path, sdk])).values()];
  if (!current || choices.some((sdk) => sdk.path === current)) return choices;
  return [...choices, { path: current, name: current }];
}

export function activatePythonStatus(): () => void {
  return registerActions([
    {
      id: "python.interpreterPopup",
      title: "Python Interpreter",
      category: "Settings",
      when: "projectOpen",
      run: () => void executeAction("file.settings", "python.interpreter"),
    },
    {
      id: "python.selectInterpreter",
      title: "Select Python Interpreter",
      category: "Settings",
      hidden: true,
      run: async ({ arg }) => {
        const project = useProject.getState().project;
        if (!project || typeof arg !== "string" || !arg) return;
        try {
          const settings = await call(
            commands.settingsSet("project", "languages.python.interpreter", JSON.stringify(arg)),
          );
          if (useProject.getState().project === project) useProject.setState({ settings });
        } catch (e) {
          notifications().notify({ level: "error", title: "Cannot select Python interpreter", body: String(e) });
        }
      },
    },
    {
      id: "python.addInterpreter",
      title: "Add New Interpreter…",
      category: "Settings",
      when: "projectOpen",
      run: () => void executeAction("file.settings", "python.interpreter"),
    },
    {
      id: "python.interpreterSettings",
      title: "Interpreter Settings…",
      category: "Settings",
      when: "projectOpen",
      run: () => void executeAction("file.settings", "python.interpreter"),
    },
  ]);
}
