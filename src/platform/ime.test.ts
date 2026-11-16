import { describe, expect, it } from "vitest";
import { isImeKey } from "@/platform/ime";

describe("isImeKey", () => {
  it("catches keys the input method is composing", () => {
    expect(isImeKey({ isComposing: true, keyCode: 78 } as KeyboardEvent)).toBe(true);
  });

  it("catches the Enter/Space that commits a candidate", () => {
    // WebKit sends the committing key after `compositionend`, with `isComposing` false and
    // keyCode 229; checking `isComposing` alone would run that Enter as a command.
    expect(isImeKey({ isComposing: false, keyCode: 229 } as KeyboardEvent)).toBe(true);
  });

  it("lets ordinary keys through", () => {
    expect(isImeKey({ isComposing: false, keyCode: 13 } as KeyboardEvent)).toBe(false);
    expect(isImeKey({ isComposing: false, keyCode: 32 } as KeyboardEvent)).toBe(false);
    expect(isImeKey({ isComposing: false, keyCode: 83 } as KeyboardEvent)).toBe(false);
  });
});
