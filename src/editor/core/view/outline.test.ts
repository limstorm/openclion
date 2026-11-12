import { describe, expect, it } from "vitest";
import type { OutlineItem, Pos } from "@/platform/ipc";
import { symbolChain } from "./outline";

/** Minimal outline node: `from`/`to` are line numbers, the whole line is inside the range. */
const item = (name: string, from: number, to: number, children: OutlineItem[] = []): OutlineItem => ({
  name,
  detail: null,
  kind: "class",
  range: { start: { line: from, col: 0 }, end: { line: to, col: 1 } },
  selection_range: { start: { line: from, col: 4 }, end: { line: from, col: 4 + name.length } },
  children,
});

const names = (items: OutlineItem[]) => items.map((i) => i.name);

const outline = [
  item("OrderController", 0, 20, [item("list", 2, 6), item("create", 8, 12, [item("body", 9, 10)])]),
  item("helper", 22, 25),
];

/** The pre-binary-search scan, kept as the reference the fast path must match. */
const before = (a: Pos, b: Pos) => a.line < b.line || (a.line === b.line && a.col <= b.col);
function linear(items: readonly OutlineItem[], line: number, col: number): OutlineItem[] {
  const pos = { line, col };
  const out: OutlineItem[] = [];
  let level = items;
  for (;;) {
    const hit = level.find((i) => before(i.range.start, pos) && before(pos, i.range.end));
    if (!hit) return out;
    out.push(hit);
    level = hit.children;
  }
}

describe("symbolChain", () => {
  it("returns the enclosing chain, outermost first", () => {
    expect(names(symbolChain(outline, 4, 2))).toEqual(["OrderController", "list"]);
    expect(names(symbolChain(outline, 9, 0))).toEqual(["OrderController", "create", "body"]);
    expect(names(symbolChain(outline, 23, 0))).toEqual(["helper"]);
  });

  it("finds nothing outside every symbol", () => {
    expect(symbolChain([], 0, 0)).toEqual([]);
    expect(symbolChain(outline, 21, 0)).toEqual([]);
    expect(symbolChain(outline, 26, 0)).toEqual([]);
  });

  it("treats ranges as inclusive on both ends, like the scan it replaces", () => {
    expect(names(symbolChain(outline, 0, 0))).toEqual(["OrderController"]);
    expect(names(symbolChain(outline, 20, 1))).toEqual(["OrderController"]);
    expect(symbolChain(outline, 20, 2)).toEqual([]);
  });

  it("picks the right sibling among many", () => {
    const wide = Array.from({ length: 500 }, (_, i) => item(`s${i}`, i * 4, i * 4 + 2));
    for (const i of [0, 1, 37, 250, 499]) {
      expect(names(symbolChain(wide, i * 4, 0))).toEqual([`s${i}`]);
      expect(symbolChain(wide, i * 4 + 3, 0)).toEqual([]);
    }
    expect(symbolChain(wide, -1, 0)).toEqual([]);
  });

  it("agrees with the linear scan on a large generated outline", () => {
    const big: OutlineItem[] = [];
    let line = 0;
    for (let i = 0; i < 400; i++) {
      const span = 1 + (i % 5);
      const children = [item(`c${i}`, line + 1, line + span - 1)];
      big.push(item(`S${i}`, line, line + span, children));
      line += span + 1;
    }
    for (let l = -2; l < line + 3; l++) {
      for (const col of [0, 1]) {
        expect(names(symbolChain(big, l, col)), `line ${l}`).toEqual(names(linear(big, l, col)));
      }
    }
  });
});
