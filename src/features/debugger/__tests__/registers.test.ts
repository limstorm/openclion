import { describe, expect, it } from "vitest";
import type { Scope, Variable } from "@/platform/ipc";
import { changedRegisters, registerRows, registerScope } from "../ui/registers";

const scope = (name: string, hint: string | null, ref = 1): Scope => ({
  name,
  variablesReference: ref,
  expensive: false,
  presentationHint: hint,
});

const variable = (name: string, value: string, typeName: string | null = null): Variable => ({
  name,
  value,
  typeName,
  variablesReference: 0,
  evaluateName: null,
});

describe("registers", () => {
  it("finds the adapter's registers scope, not locals or globals", () => {
    const scopes = [scope("Locals", "locals", 10), scope("Registers", "registers", 11), scope("Globals", null, 12)];
    expect(registerScope(scopes)?.name).toBe("Registers");
    expect(registerScope([scope("Locals", "locals")])).toBeUndefined();
  });

  it("keeps adapter order and types", () => {
    expect(registerRows([variable("rax", "0x1", "uint64_t"), variable("eflags", "0x246")])).toEqual([
      { name: "rax", value: "0x1", type: "uint64_t" },
      { name: "eflags", value: "0x246", type: null },
    ]);
  });

  it("marks the registers whose value changed since the previous stop", () => {
    const before = registerRows([variable("rax", "0x1"), variable("rip", "0x1000")]);
    const after = registerRows([variable("rax", "0x2"), variable("rip", "0x1000")]);
    expect([...changedRegisters(before, after)]).toEqual(["rax"]);
    expect(changedRegisters([], after).size).toBe(0);
  });
});
