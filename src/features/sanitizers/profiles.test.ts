import { describe, expect, it } from "vitest";
import { hasSanitizerFlags, withSanitizerFlags } from "./profiles";

describe("sanitizer CMake flags", () => {
  it("adds both compiler flag sets without dropping other arguments", () => {
    const args = withSanitizerFlags(["-DFEATURE=ON"], true);
    expect(args[0]).toBe("-DFEATURE=ON");
    expect(args).toHaveLength(3);
    expect(args[1]).toMatch(/^-DCMAKE_C_FLAGS=.*-fsanitize=address,undefined/);
    expect(args[2]).toMatch(/^-DCMAKE_CXX_FLAGS=.*-fno-omit-frame-pointer/);
    expect(hasSanitizerFlags(args)).toBe(true);
  });

  it("removes only the sanitizer arguments when disabled", () => {
    const once = withSanitizerFlags(["-DFEATURE=ON"], true);
    expect(withSanitizerFlags(once, false)).toEqual(["-DFEATURE=ON"]);
    expect(withSanitizerFlags(["-DFEATURE=ON"], false)).toEqual(["-DFEATURE=ON"]);
    expect(hasSanitizerFlags(["-DFEATURE=ON"])).toBe(false);
  });

  it("does not duplicate flags when applied twice", () => {
    const args = withSanitizerFlags(withSanitizerFlags([], true), true);
    expect(args).toHaveLength(2);
  });
});
