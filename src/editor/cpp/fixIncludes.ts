/**
 * "Fix Missing Includes" (CLion's include fixer): clangd reports one quick fix
 * per unresolved symbol (`Add #include <vector> for 'std::vector'`); this applies
 * every one of them in a row, re-querying after each edit.
 */
import type { EditorView } from "@codemirror/view";
import type { CodeAction } from "@/platform/ipc";
import { registerActions } from "@/platform/actions";
import { activeEditor } from "@/editor/api";
import { showHint } from "@/editor/lsp/hint";
import { applyAllKind } from "@/editor/lsp/intentions";
import { tx } from "@/platform/i18n";

const EDITOR_WHEN = "textEditorActive && (editorFocus || popupOpen || !toolWindowFocus)";

/** clangd's include quick fixes, and nothing else the quick-fix list offers. */
export function isIncludeFix(action: Pick<CodeAction, "title">): boolean {
  return /#include\b|add include|include the header/i.test(action.title);
}

/** Applies every missing-include fix at the caret's file; false when there was none. */
export async function fixIncludes(view: EditorView): Promise<boolean> {
  const applied = await applyAllKind(view, "quickfix", isIncludeFix);
  if (applied > 0) {
    showHint(view, tx`Added ${String(applied)} include(s).`);
    return true;
  }
  showHint(view, tx("No missing include was reported here."));
  return false;
}

export function registerIncludeFixer(): () => void {
  return registerActions([
    {
      id: "cpp.fixIncludes",
      title: "Fix Missing Includes",
      description: "Adds the includes clangd reports as missing in this file",
      category: "Code",
      keywords: ["include", "fix", "missing", "clangd", "cpp", "hpp"],
      when: EDITOR_WHEN,
      run: () => {
        const editor = activeEditor();
        if (editor) void fixIncludes(editor.view);
      },
    },
  ]);
}
