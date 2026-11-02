import { describe, expect, it } from "vitest";
import { isIncludeFix } from "./fixIncludes";

describe("include fixer", () => {
  it("recognizes clangd's missing-include quick fixes", () => {
    for (const title of [
      "Add #include <vector> for 'std::vector'",
      "Add #include \"greeter.h\" for 'Greeter'",
      "Add include for 'size_t'",
      "Include the header <cstdint> for 'uint32_t'",
    ]) {
      expect(isIncludeFix({ title }), title).toBe(true);
    }
  });

  it("leaves other quick fixes alone", () => {
    for (const title of [
      "Declare 'foo'",
      "Create method 'bar'",
      "Change variable type to 'int'",
      "Fix ambiguous conversion",
    ]) {
      expect(isIncludeFix({ title }), title).toBe(false);
    }
  });
});
