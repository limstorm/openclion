import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const { debugScopes, debugVariables, debugDisassemble } = vi.hoisted(() => ({
  debugScopes: vi.fn(() =>
    Promise.resolve({
      status: "ok" as const,
      data: [
        { name: "Locals", variablesReference: 1, expensive: false, presentationHint: "locals" },
        { name: "Registers", variablesReference: 2, expensive: false, presentationHint: "registers" },
      ],
    }),
  ),
  debugVariables: vi.fn(() =>
    Promise.resolve({
      status: "ok" as const,
      data: [
        { name: "rax", value: "0x0000000000000001", typeName: "uint64_t", variablesReference: 0, evaluateName: null },
        { name: "rip", value: "0x00005555555551a0", typeName: "uint64_t", variablesReference: 0, evaluateName: null },
      ],
    }),
  ),
  debugDisassemble: vi.fn(() =>
    Promise.resolve({
      status: "ok" as const,
      data: [
        {
          address: "0x0000555555555190",
          instruction: "push rbp",
          symbol: "main",
          file: "/p/src/main.cpp",
          line: 4,
        },
        { address: "0x00005555555551a0", instruction: "mov rbp, rsp", symbol: null, file: null, line: null },
      ],
    }),
  ),
}));

vi.mock("@/platform/ipc", async (importOriginal) => {
  const real = await importOriginal<typeof import("@/platform/ipc")>();
  return {
    ...real,
    commands: { ...real.commands, debugScopes, debugVariables, debugDisassemble },
  };
});

import { useDebug, type DebugSession } from "../state";
import { DebugToolWindow } from "../ui/DebugToolWindow";

const session = (disassemble: boolean): DebugSession => ({
  id: 1,
  name: "main",
  adapter: "lldb-dap",
  parent: null,
  launch: null,
  process: null,
  status: "paused",
  stoppedThread: 1,
  stopReason: "breakpoint",
  stopDescription: null,
  threads: [{ id: 1, name: "MainThread" }],
  selectedThread: 1,
  frames: [
    {
      id: 7,
      name: "main",
      path: "/p/src/main.cpp",
      line: 4,
      column: 0,
      presentationHint: null,
      instructionPointer: "0x00005555555551a0",
    },
  ],
  selectedFrame: 7,
  hits: 1,
  exitCode: null,
  caps: {
    restartFrame: false,
    stepInTargets: false,
    functionBreakpoints: false,
    dataBreakpoints: false,
    readMemory: true,
    disassemble,
    setValue: true,
    conditionalBreakpoints: true,
    hitConditionalBreakpoints: true,
    logPoints: true,
  },
});

describe("registers view", () => {
  it("lists the registers scope of the selected frame", async () => {
    useDebug.setState({ sessions: { 1: session(true) }, active: 1 });
    render(<DebugToolWindow />);
    fireEvent.click(screen.getByRole("tab", { name: "Registers" }));
    await waitFor(() => expect(screen.getByRole("table", { name: "Registers" })).toBeTruthy());
    expect(debugScopes).toHaveBeenCalledWith(1, 7);
    expect(debugVariables).toHaveBeenCalledWith(1, 2);
    expect(screen.getByRole("cell", { name: "rax" })).toBeTruthy();
    expect(screen.getByRole("cell", { name: "0x0000000000000001" })).toBeTruthy();
  });
});

describe("disassembly view", () => {
  it("disassembles around the program counter and marks it", async () => {
    useDebug.setState({ sessions: { 1: session(true) }, active: 1 });
    render(<DebugToolWindow />);
    fireEvent.click(screen.getByRole("tab", { name: "Disassembly" }));
    await waitFor(() => expect(screen.getByRole("table", { name: "Disassembly" })).toBeTruthy());
    expect(debugDisassemble).toHaveBeenCalledWith(1, "0x00005555555551a0", -20, 120);
    expect(screen.getByRole("cell", { name: "push rbp" })).toBeTruthy();
    expect(screen.getByRole("cell", { name: "mov rbp, rsp" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "main.cpp:5" })).toBeTruthy();
  });

  it("says so when the adapter cannot disassemble", () => {
    useDebug.setState({ sessions: { 1: session(false) }, active: 1 });
    render(<DebugToolWindow />);
    fireEvent.click(screen.getByRole("tab", { name: "Disassembly" }));
    expect(screen.getByText("Disassembly is not available for this debugger")).toBeTruthy();
  });
});
