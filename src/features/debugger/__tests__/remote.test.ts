import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { RunConfig } from "@/platform/ipc";
import { registerRunConfigType } from "@/workbench/contributions";

const remote = await import("../remote");
const { missingParams, newConfigOfType, isDebugOnly, terminalApplies, withParams } =
  await import("@/features/run/types");
const { validateConfig } = await import("@/features/run/configs");

const { REMOTE_TYPES, goRemoteType, pythonRemoteType, nodeRemoteType, remoteAttachConfig } = remote;

function configOf(type: (typeof REMOTE_TYPES)[number], params: Record<string, string> = {}): RunConfig {
  const c = newConfigOfType(type, []);
  return withParams({ ...c, name: "Remote" }, { ...c.params, ...params }, type);
}

let disposers: Array<() => void> = [];
beforeEach(() => {
  disposers = REMOTE_TYPES.map(registerRunConfigType);
});
afterEach(() => disposers.forEach((d) => d()));

describe("remote debug configuration types", () => {
  it.each(REMOTE_TYPES.map((t) => [t.id, t] as const))(
    "%s is debug-only, launches nothing and is valid by default",
    (_, type) => {
      const c = configOf(type);
      expect(type.debugOnly).toBe(true);
      expect(isDebugOnly(c)).toBe(true);
      expect(c.debugAdapter).toBe(type.debugAdapter);
      expect(c.command).toBe("");
      expect(terminalApplies(c, type)).toBe(false);
      expect(validateConfig(c, [c])).toEqual({});
      expect(missingParams(c, type)).toEqual({});
    },
  );

  it.each(REMOTE_TYPES.map((t) => [t.id, t] as const))("%s rejects a bad port and an empty host", (_, type) => {
    expect(missingParams(configOf(type, { port: "70000" }), type)).toEqual({
      port: "Port must be a number from 1 to 65535",
    });
    expect(missingParams(configOf(type, { port: "abc" }), type).port).toBeDefined();
    expect(missingParams(configOf(type, { host: "" }), type)).toEqual({ host: "Host is required" });
  });
});

describe("remote attach requests", () => {
  it("Go Remote connects to dlv --headless and maps the source root", () => {
    const l = remoteAttachConfig(configOf(goRemoteType, { host: "build", remoteRoot: "/go/src/app" }), "/p")!;
    expect(l).toMatchObject({ adapter: "delve", request: "attach", cwd: "/p" });
    expect(JSON.parse(l.extraJson!)).toEqual({
      debugServer: "build:2345",
      mode: "remote",
      substitutePath: [{ from: "/p", to: "/go/src/app" }],
    });
    const plain = JSON.parse(remoteAttachConfig(configOf(goRemoteType), "/p")!.extraJson!);
    expect(plain).toEqual({ debugServer: "localhost:2345", mode: "remote" });
  });

  it("Python remote connects to debugpy --listen with pathMappings", () => {
    const l = remoteAttachConfig(
      configOf(pythonRemoteType, { host: "::1", localRoot: "svc", remoteRoot: "/app" }),
      "/p",
    )!;
    expect(l.adapter).toBe("debugpy");
    expect(JSON.parse(l.extraJson!)).toMatchObject({
      debugServer: "[::1]:5678",
      pathMappings: [{ localRoot: "/p/svc", remoteRoot: "/app" }],
    });
  });

  it("Attach to Godot connects to the editor's debug adapter for the project", () => {
    const l = remoteAttachConfig(configOf(remote.godotAttachType), "/p/game")!;
    expect(l).toMatchObject({ adapter: "godot", request: "attach", cwd: "/p/game" });
    expect(JSON.parse(l.extraJson!)).toEqual({ debugServer: "localhost:6006", project: "/p/game" });
    const custom = remoteAttachConfig(configOf(remote.godotAttachType, { port: "6010" }), "/p")!;
    expect(JSON.parse(custom.extraJson!).debugServer).toBe("localhost:6010");
  });

  it("Attach to Node.js attaches js-debug to the inspector with local/remote roots", () => {
    const l = remoteAttachConfig(configOf(nodeRemoteType, { host: "web", remoteRoot: "/usr/src/app" }), "/p")!;
    expect(l.adapter).toBe("js-debug");
    expect(JSON.parse(l.extraJson!)).toMatchObject({
      type: "pwa-node",
      address: "web",
      port: 9229,
      localRoot: "/p",
      remoteRoot: "/usr/src/app",
    });
  });

  it("start-the-remote-side hints follow the port", () => {
    expect(remote.delveServerCommand({ port: "4000" })).toBe(
      "dlv debug --headless --listen=:4000 --api-version=2 --accept-multiclient",
    );
    expect(remote.debugpyServerCommand({ port: "5679" })).toContain("--listen 0.0.0.0:5679");
    expect(remote.nodeInspectCommand({})).toBe("node --inspect=0.0.0.0:9229 app.js");
  });
});
