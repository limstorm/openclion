import { describe, expect, it } from "vitest";
import { methodAt } from "../breakpoints/symbols";

describe("method declarations", () => {
  it("names functions for debugpy, Delve, LLDB and js-debug", () => {
    expect(methodAt("python", "class A:\n    async def run(self, n):\n        pass", 1)?.name).toBe("run");
    const go =
      "package shop\n\nfunc Parse(s string) int {\n}\nfunc (o *Order) Total() int {\n}\nfunc (o Order) Name() string {\n}";
    expect(methodAt("go", go, 2)?.name).toBe("shop.Parse");
    expect(methodAt("go", go, 4)?.name).toBe("shop.(*Order).Total");
    expect(methodAt("go", go, 6)?.name).toBe("shop.Order.Name");
    expect(methodAt("rust", "pub(crate) async fn parse(text: &str) -> i32 {", 0)?.name).toBe("parse");
    expect(methodAt("c", "static int parse(const char *text) {", 0)?.name).toBe("parse");
    expect(methodAt("c", "    if (x) {", 0)).toBeNull();
    expect(methodAt("c", "    total += parse(buf);", 0)).toBeNull();
    expect(methodAt("typescript", "export async function load(id: number) {", 0)?.name).toBe("load");
    expect(methodAt("plaintext", "def x():", 0)).toBeNull();
  });
});
