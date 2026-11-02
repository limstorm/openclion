/**
 * CLion's "Go to Related Symbol": switch a C/C++ source to its header and back.
 * The backend resolves the conventional `src/` ↔ `include/` layout and falls
 * back to the project index; this module keeps the decision testable.
 */
import { tx } from "@/platform/i18n";

export interface RelatedSymbolDeps {
  /** Candidate files for `path`, best first (backend `cpp_related_files`). */
  related(path: string): Promise<string[]>;
  open(path: string): Promise<void>;
  hint(message: string): void;
}

/** Opens the related header/source of `path`; false when there is none. */
export async function switchToRelated(path: string, deps: RelatedSymbolDeps): Promise<boolean> {
  const candidates = await deps.related(path);
  const target = candidates[0];
  if (!target) {
    deps.hint(tx`No header or source with the same name was found for ${basename(path)}.`);
    return false;
  }
  await deps.open(target);
  return true;
}

function basename(path: string): string {
  const cut = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return cut >= 0 ? path.slice(cut + 1) : path;
}
