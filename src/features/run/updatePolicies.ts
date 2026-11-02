import type { RunConfig } from "@/platform/ipc";
import type { RunConfigTypeField } from "@/workbench/contributions";

export type UpdateMode = "resources" | "classes" | "redeploy" | "restart";

export const updatePolicyDefaults = { onUpdate: "ask", onDeactivation: "none" };

const incrementalOptions = [
  { value: "resources", label: "Update resources" },
  { value: "classes", label: "Update classes and resources" },
];

/** "Update Application" policies of a local Tomcat server configuration. */
export function updatePolicyFields(): RunConfigTypeField[] {
  return [
    {
      key: "onUpdate",
      label: "On 'Update' action",
      control: "select",
      options: [
        { value: "ask", label: "Show chooser" },
        ...incrementalOptions,
        { value: "redeploy", label: "Redeploy" },
        { value: "restart", label: "Restart" },
      ],
    },
    {
      key: "onDeactivation",
      label: "On frame deactivation",
      control: "select",
      options: [{ value: "none", label: "Do nothing" }, ...incrementalOptions],
      hint: "When all project windows lose focus to another application",
    },
  ];
}

/** Configurations "Update Application" applies to: local Tomcat servers. */
export function isUpdatable(config: RunConfig): boolean {
  return !config.target && config.kind === "tomcat";
}

export function updateAllowed(config: RunConfig, mode: string): mode is UpdateMode {
  return (
    isUpdatable(config) &&
    (mode === "resources" || mode === "classes" || mode === "restart" || mode === "redeploy") &&
    !((mode === "resources" || mode === "classes") && config.params?.deployment === "war")
  );
}
