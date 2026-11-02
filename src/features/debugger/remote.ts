/**
 * Remote debug run configurations: the debugger attaches to a process started
 * elsewhere (another machine, a container) over host:port, like GoLand's Go Remote,
 * PyCharm's remote debugpy and IDEA's Attach to Node.js.
 * Only Debug applies to them (`debugOnly`); they launch no process.
 */
import type { LaunchConfig, RunConfig } from "@/platform/ipc";
import { workflowSettings } from "@/platform/workflowSettings";
import type { RunConfigTypeContribution, RunConfigTypeField } from "@/workbench/contributions";

export const GO_REMOTE = "go-remote";
export const PYTHON_REMOTE = "python-remote";
export const NODE_REMOTE = "node-remote";
export const GODOT_ATTACH = "godot-attach";

type Params = Readonly<Record<string, string>>;

/** A TCP port, or null. */
export function parsePort(value: string | undefined): number | null {
  const v = (value ?? "").trim();
  if (!/^\d+$/.test(v)) return null;
  const n = Number(v);
  return n >= 1 && n <= 65535 ? n : null;
}

const hostField: RunConfigTypeField = { key: "host", label: "Host", mono: true, required: true };
const portField: RunConfigTypeField = {
  key: "port",
  label: "Port",
  mono: true,
  required: true,
  validate: (v) => (parsePort(v) == null ? "Port must be a number from 1 to 65535" : null),
};

const hostOf = (p: Params) => p.host?.trim() || "localhost";
const portOf = (p: Params, fallback: number) => parsePort(p.port) ?? fallback;

// ---------------------------------------------------------------------------
// Go Remote (Delve headless server)

/** How to start the remote side, as GoLand shows it. */
export function delveServerCommand(p: Params): string {
  return `dlv debug --headless --listen=:${portOf(p, 2345)} --api-version=2 --accept-multiclient`;
}

const localRootField: RunConfigTypeField = {
  key: "localRoot",
  label: "Local root",
  mono: true,
  placeholder: "Project root",
};

const remoteRootField = (placeholder: string): RunConfigTypeField => ({
  key: "remoteRoot",
  label: "Remote root",
  mono: true,
  placeholder,
  hint: "Where the project's sources are on the remote machine, when the paths differ",
});

export const goRemoteType: RunConfigTypeContribution = {
  id: GO_REMOTE,
  label: "Go Remote",
  icon: "file-go",
  description: "Attach to a Delve server (dlv --headless) here or on another machine",
  defaults: { host: "localhost", port: "2345" },
  debugOnly: true,
  debugAdapter: "delve",
  fields: [
    hostField,
    portField,
    {
      key: "hint",
      label: "Start the remote side with",
      control: "output",
      mono: true,
      value: delveServerCommand,
      hint: "Delve 1.7.3 or later; dlv exec ./app or dlv attach <pid> take the same flags",
    },
    localRootField,
    remoteRootField("e.g. /go/src/app"),
  ],
  commandLine: () => null,
};

// ---------------------------------------------------------------------------
// Python remote (debugpy --listen)

export function debugpyServerCommand(p: Params): string {
  return `python -m debugpy --listen 0.0.0.0:${portOf(p, 5678)} --wait-for-client app.py`;
}

export const pythonRemoteType: RunConfigTypeContribution = {
  id: PYTHON_REMOTE,
  label: "Python Remote Debug (debugpy)",
  icon: "file-python",
  description: "Attach to a Python process listening with debugpy here or on another machine",
  defaults: { host: "localhost", port: "5678" },
  debugOnly: true,
  debugAdapter: "debugpy",
  fields: [
    hostField,
    portField,
    {
      key: "hint",
      label: "Start the remote side with",
      control: "output",
      mono: true,
      value: debugpyServerCommand,
      hint: "pip install debugpy on the remote machine",
    },
    localRootField,
    remoteRootField("e.g. /app"),
  ],
  commandLine: () => null,
};

// ---------------------------------------------------------------------------
// Attach to Node.js (inspector)

export function nodeInspectCommand(p: Params): string {
  return `node --inspect=0.0.0.0:${portOf(p, 9229)} app.js`;
}

export const nodeRemoteType: RunConfigTypeContribution = {
  id: NODE_REMOTE,
  label: "Attach to Node.js",
  icon: "file-js",
  description: "Attach to a Node.js process started with --inspect here or on another machine",
  defaults: { host: "localhost", port: "9229" },
  debugOnly: true,
  debugAdapter: "js-debug",
  fields: [
    hostField,
    portField,
    {
      key: "hint",
      label: "Start the remote side with",
      control: "output",
      mono: true,
      value: nodeInspectCommand,
    },
    localRootField,
    remoteRootField("e.g. /usr/src/app"),
  ],
  commandLine: () => null,
};

// ---------------------------------------------------------------------------
// Attach to Godot (the editor's debug adapter, Editor Settings › Network › Debug Adapter)

export const godotAttachType: RunConfigTypeContribution = {
  id: GODOT_ATTACH,
  label: "Attach to Godot",
  icon: "file-gdscript",
  description: "Debug the game the Godot editor is running, through the editor's debug adapter",
  defaults: { host: "localhost", port: "6006" },
  debugOnly: true,
  debugAdapter: "godot",
  fields: [
    hostField,
    portField,
    {
      key: "hint",
      label: "Before attaching",
      control: "output",
      value: () => "Open the project in the Godot editor and run a scene (F5 / F6)",
    },
  ],
  commandLine: () => null,
};

export const REMOTE_TYPES: readonly RunConfigTypeContribution[] = [
  goRemoteType,
  pythonRemoteType,
  nodeRemoteType,
  godotAttachType,
];

export const isRemoteConfig = (c: Pick<RunConfig, "kind">) => REMOTE_TYPES.some((t) => t.id === c.kind);

// ---------------------------------------------------------------------------
// Attach requests

function isAbsolute(p: string) {
  return p.startsWith("/") || /^[A-Za-z]:[\\/]/.test(p);
}

function join(base: string, p: string) {
  if (isAbsolute(p)) return p;
  const rel = p.replace(/^\.\//, "");
  return rel === "." || rel === "" ? base : `${base.replace(/[\\/]$/, "")}/${rel}`;
}

/** Local root (absolute) ↔ remote root, or null when the paths are the same on both sides. */
function rootMapping(p: Params, root: string): { local: string; remote: string } | null {
  const remote = p.remoteRoot?.trim();
  if (!remote) return null;
  const local = p.localRoot?.trim() ? join(root, p.localRoot.trim()) : root;
  return { local, remote };
}

/** `host:port` for `debugServer` (IPv6 literals in brackets). */
export function serverAddress(host: string, port: number): string {
  return `${host.includes(":") && !host.startsWith("[") ? `[${host}]` : host}:${port}`;
}

function attachBase(rc: RunConfig, adapter: string, root: string, extra: Record<string, unknown>): LaunchConfig {
  return {
    name: rc.name,
    adapter,
    request: "attach",
    program: null,
    args: [],
    cwd: root,
    env: {},
    extraJson: JSON.stringify(extra),
  };
}

/**
 * The attach request of a Go / Python / Node.js remote configuration. Delve's headless
 * server and `debugpy --listen` speak DAP themselves (`debugServer` connects there);
 * js-debug runs here and connects to the inspector.
 */
export function remoteAttachConfig(rc: RunConfig, root: string): LaunchConfig | null {
  const p = rc.params ?? {};
  const host = hostOf(p);
  const map = rootMapping(p, root);
  const skip = workflowSettings().debugger.skipLibraryCode;
  switch (rc.kind) {
    case GO_REMOTE:
      return attachBase(rc, "delve", root, {
        debugServer: serverAddress(host, portOf(p, 2345)),
        mode: "remote",
        ...(map ? { substitutePath: [{ from: map.local, to: map.remote }] } : {}),
      });
    case PYTHON_REMOTE:
      return attachBase(rc, "debugpy", root, {
        debugServer: serverAddress(host, portOf(p, 5678)),
        justMyCode: skip,
        ...(map ? { pathMappings: [{ localRoot: map.local, remoteRoot: map.remote }] } : {}),
      });
    case GODOT_ATTACH:
      return attachBase(rc, "godot", root, { debugServer: serverAddress(host, portOf(p, 6006)), project: root });
    case NODE_REMOTE:
      return attachBase(rc, "js-debug", root, {
        type: "pwa-node",
        address: host,
        port: portOf(p, 9229),
        ...(map ? { localRoot: map.local, remoteRoot: map.remote } : {}),
        skipFiles: skip ? ["<node_internals>/**", "**/node_modules/**"] : [],
      });
    default:
      return null;
  }
}
