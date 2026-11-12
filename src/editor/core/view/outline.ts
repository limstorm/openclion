import type { OutlineItem, Pos } from "@/platform/ipc";

const before = (a: Pos, b: Pos) => a.line < b.line || (a.line === b.line && a.col <= b.col);

/** Index of the last item starting at or before `pos`, or -1 when every item starts after it. */
function lastStartingAtOrBefore(items: readonly OutlineItem[], pos: Pos): number {
  let lo = 0;
  let hi = items.length - 1;
  let hit = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (before(items[mid]!.range.start, pos)) {
      hit = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return hit;
}

/**
 * Outline items enclosing (line, col), outermost first.
 *
 * Items are in document order and siblings never overlap — `ide-language`'s
 * outline builder sorts a node's children by start offset
 * (`crates/ide-language/src/syntax/outline.rs`) — so the last sibling starting at
 * or before the caret is the only one that can enclose it. Binary search keeps a
 * caret move O(depth × log width) rather than one scan of every top-level symbol:
 * on a 12k-line C++ file (8k structs and functions) the scan cost ~0.1 ms per
 * call, and the breadcrumbs run it on every caret move and scroll-driven status
 * update.
 */
export function symbolChain(items: readonly OutlineItem[], line: number, col: number): OutlineItem[] {
  const pos = { line, col };
  const out: OutlineItem[] = [];
  let level: readonly OutlineItem[] = items;
  for (;;) {
    const at = lastStartingAtOrBefore(level, pos);
    if (at < 0) return out;
    const hit = level[at]!;
    if (!before(pos, hit.range.end)) return out;
    out.push(hit);
    level = hit.children;
  }
}
