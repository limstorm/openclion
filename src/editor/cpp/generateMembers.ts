/**
 * CLion's Generate › Override/Implement Methods: a C/C++ class under the caret
 * in a header gains stubs for the base-class virtual members it does not define
 * (pure virtuals are mandatory), inserted inside the body before the closing `};`.
 * The file opens on the first generated stub.
 *
 * `requestMembers` holds the decision so it is testable; the action wires it to
 * the editor and the backend.
 */
import type { EditorView } from "@codemirror/view";
import { activeEditor } from "@/editor/api";
import { showHint } from "@/editor/lsp/hint";
import { docOf } from "@/editor/lsp/lsp";
import { offsetToPos } from "@/editor/lsp/positions";
import { registerActions } from "@/platform/actions";
import { call, commands, isTauri, type DocId, type Pos } from "@/platform/ipc";
import { isVirtualUri } from "@/platform/virtualDocs";
import { editorService } from "@/workbench/contributions";
import { isCppHeader, type GeneratedDefinition } from "./generateDefinition";

const EDITOR_WHEN = "textEditorActive && (editorFocus || popupOpen || !toolWindowFocus)";

export interface MembersDeps {
  generate(doc: DocId, pos: Pos): Promise<GeneratedDefinition>;
  open(file: string, line: number): Promise<void>;
  hint(message: string): void;
}

/**
 * Generates the missing overrides for the class at `pos`; false when the file
 * is not a header (the caller then shows its own "nothing to generate" hint).
 */
export async function requestMembers(path: string, doc: DocId, pos: Pos, deps: MembersDeps): Promise<boolean> {
  if (!isCppHeader(path)) return false;
  try {
    const generated = await deps.generate(doc, pos);
    await deps.open(generated.file, generated.line);
  } catch (e) {
    deps.hint(e instanceof Error ? e.message : String(e));
  }
  return true;
}

async function generateFromEditor(view: EditorView): Promise<boolean> {
  const ctx = docOf(view);
  if (!ctx || !isTauri() || isVirtualUri(ctx.path)) return false;
  const pos = offsetToPos(view.state.doc, view.state.selection.main.head);
  return requestMembers(ctx.path, ctx.doc, pos, {
    generate: (doc, at) => call(commands.cppGenerateMembers(doc, at)),
    open: async (file, line) => {
      await editorService().openFile(file, { selection: { line: line - 1, col: 0 }, focus: true });
    },
    hint: (message) => showHint(view, message, { tone: "error" }),
  });
}

/** Generate › Override/Implement Methods for C/C++ (syntax-based, no server needed). */
export function registerCppMemberActions(): () => void {
  return registerActions([
    {
      id: "cpp.generateMembers",
      title: "Generate Override/Implement Methods",
      description: "Creates stubs for the base-class virtual methods a C/C++ class does not override",
      category: "Code",
      keywords: ["generate", "override", "implement", "virtual", "cpp", "stub"],
      when: EDITOR_WHEN,
      run: () => {
        const editor = activeEditor();
        if (editor) void generateFromEditor(editor.view);
      },
    },
  ]);
}
