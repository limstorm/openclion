import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";

const { readMemory } = vi.hoisted(() => ({
  readMemory: vi.fn(() =>
    Promise.resolve({
      status: "ok" as const,
      data: {
        address: "0x1000",
        data: Array.from({ length: 16 }, (_, i: number) => 0x41 + i),
        unreadableBytes: 0,
      },
    }),
  ),
}));

vi.mock("@/platform/ipc", async (importOriginal) => {
  const real = await importOriginal<typeof import("@/platform/ipc")>();
  return { ...real, commands: { ...real.commands, debugReadMemory: readMemory } };
});

import { useDebug, type DebugSession } from "../state";
import { DebugToolWindow } from "../ui/DebugToolWindow";

const session: DebugSession = {
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
      instructionPointer: "0x1000",
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
    disassemble: true,
    setValue: true,
    conditionalBreakpoints: true,
    hitConditionalBreakpoints: true,
    logPoints: true,
  },
};

describe("memory view", () => {
  it("reads the typed address and renders hex and ASCII cells", async () => {
    readMemory.mockClear();
    useDebug.setState({ sessions: { 1: session }, active: 1 });
    render(<DebugToolWindow />);
    fireEvent.click(screen.getByRole("tab", { name: "Memory" }));

    const input = screen.getByLabelText("Memory address or expression");
    fireEvent.change(input, { target: { value: "0x1000" } });
    fireEvent.click(screen.getByRole("button", { name: "Go" }));

    await waitFor(() => expect(screen.getByRole("table", { name: "Memory" })).toBeTruthy());
    expect(readMemory).toHaveBeenCalledWith(1, "0x1000", 0, 512);
    expect(screen.getByRole("cell", { name: "0x1000" })).toBeTruthy();
    expect(screen.getByRole("cell", { name: "41" })).toBeTruthy();
    expect(screen.getByRole("cell", { name: "50" })).toBeTruthy();
    expect(screen.getByRole("cell", { name: "ABCDEFGHIJKLMNOP" })).toBeTruthy();
  });

  it("says so when the adapter cannot read memory", () => {
    act(() =>
      useDebug.setState({
        sessions: { 1: { ...session, caps: { ...session.caps!, readMemory: false } } },
        active: 1,
      }),
    );
    render(<DebugToolWindow />);
    fireEvent.click(screen.getByRole("tab", { name: "Memory" }));
    expect(screen.getByText("Memory is not available for this debugger")).toBeTruthy();
  });
});
