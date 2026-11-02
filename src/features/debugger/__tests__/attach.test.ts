import { describe, expect, it } from "vitest";
import type { AttachableProcess } from "@/platform/ipc";
import { attachConfig, attachDetail, attachRows, matchesFilter } from "../attach";

const ADAPTERS: Record<string, string> = {
  node: "js-debug",
  python: "debugpy",
  go: "delve",
  native: "codelldb",
};

const proc = (p: Partial<AttachableProcess> & Pick<AttachableProcess, "pid" | "kind">): AttachableProcess => ({
  name: p.kind,
  commandLine: `${p.kind} app`,
  adapter: ADAPTERS[p.kind]!,
  host: null,
  port: null,
  ...p,
});

const list = [
  proc({ pid: 30, kind: "native", name: "a.out", commandLine: "./a.out" }),
  proc({ pid: 20, kind: "python", name: "python3", commandLine: "python3 worker.py" }),
  proc({
    pid: 21,
    kind: "python",
    name: "python3",
    commandLine: "python3 -m debugpy --listen 5678 api.py",
    port: 5678,
  }),
  proc({ pid: 25, kind: "node", name: "node", commandLine: "node server.js" }),
  proc({ pid: 26, kind: "go", name: "gosrv", commandLine: "./gosrv" }),
];

describe("Attach to Process popup model", () => {
  it("groups processes by debugger with counts and skips kinds it cannot attach", () => {
    const rows = attachRows(list, "");
    expect(rows.filter((r) => r.kind === "group").map((r) => r.kind === "group" && `${r.label}:${r.count}`)).toEqual([
      "Node.js:1",
      "Python (debugpy):2",
      "Go (Delve):1",
      "Native (LLDB):1",
    ]);
    expect(rows[0]!.kind).toBe("group");
    expect(rows[1]!.kind === "process" && rows[1]!.process.pid).toBe(25);
    expect(rows.some((r) => r.kind === "process" && r.process.pid === 10)).toBe(false);
  });

  it("filters on pid, name and command line (all words)", () => {
    expect(matchesFilter(list[1]!, "worker")).toBe(true);
    expect(matchesFilter(list[1]!, "20")).toBe(true);
    expect(matchesFilter(list[1]!, "python api")).toBe(false);
    const rows = attachRows(list, "debugpy");
    expect(rows.map((r) => r.key)).toEqual(["group:python", "python:21"]);
    expect(attachRows(list, "nothing-matches")).toEqual([]);
  });

  it("describes how each process is reached", () => {
    expect(attachDetail(list[2]!)).toBe("localhost:5678");
    expect(attachDetail(list[1]!)).toBe("pid 20");
  });

  it("builds the attach request of each debugger", async () => {
    const extra = async (p: AttachableProcess) => JSON.parse((await attachConfig(p, "/proj")).extraJson!);
    // debugpy: inject by pid, or talk to a `--listen` adapter directly.
    expect(await extra(list[1]!)).toMatchObject({ processId: 20 });
    expect(await extra(list[2]!)).toMatchObject({ debugServer: 5678 });
    // js-debug: the backend opens the inspector of a process started without --inspect.
    expect(await extra(list[3]!)).toMatchObject({ type: "pwa-node", processId: "25" });
    expect(await extra(proc({ pid: 27, kind: "node", port: 9230 }))).toMatchObject({
      port: 9230,
      address: "localhost",
    });
    expect(await extra(list[4]!)).toEqual({ mode: "local", processId: 26 });
    const native = await attachConfig(list[0]!, "/proj");
    expect(native).toMatchObject({ adapter: "codelldb", request: "attach", name: "a.out (30)", cwd: "/proj" });
    expect(JSON.parse(native.extraJson!)).toEqual({ pid: 30 });
  });
});
