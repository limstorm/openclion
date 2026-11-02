/**
 * CLion's Generate › Definition: the C/C++ declaration under the caret in a
 * header gets its stub appended to the matching source file (the same pair
 * `Go to Related Symbol` resolves), then that file opens on the new stub.
 *
 * `requestDefinition` holds the decision so it is testable; the action wires it
 * to the editor and the backend.
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

const EDITOR_WHEN = "textEditorActive && (editorFocus || popupOpen || !toolWindowFocus)";
/** Extensions a definition can be generated from (the header side). */
const HEADER = /\.(h|hh|hpp|hxx|h\+{2}|inl|ipp|tpp)$/i;

export function isCppHeader(path: string): boolean {
  return HEADER.test(path);
}

export interface GeneratedDefinition {
  file: string;
  /** 1-based line of the generated stub. */
  line: number;
}

export interface DefinitionDeps {
  generate(doc: DocId, pos: Pos): Promise<GeneratedDefinition>;
  open(file: string, line: number): Promise<void>;
  hint(message: string): void;
}

/**
 * Generates the definition for the declaration at `pos`; false when the file is
 * not a header (the caller then shows its own "nothing to generate" hint).
 */
export async function requestDefinition(path: string, doc: DocId, pos: Pos, deps: DefinitionDeps): Promise<boolean> {
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
  return requestDefinition(ctx.path, ctx.doc, pos, {
    generate: (doc, at) => call(commands.cppGenerateDefinition(doc, at, null)),
    open: async (file, line) => {
      await editorService().openFile(file, { selection: { line: line - 1, col: 0 }, focus: true });
    },
    hint: (message) => showHint(view, message, { tone: "error" }),
  });
}

/** Generate › Definition (`code.generate` falls back to it when clangd has none). */
export function registerCppActions(): () => void {
  return registerActions([
    {
      id: "cpp.generateDefinition",
      title: "Generate Definition",
      description: "Creates the definition of a C/C++ declaration in the matching source file",
      category: "Code",
      keywords: ["generate", "definition", "cpp", "header", "stub", "implement"],
      when: EDITOR_WHEN,
      run: () => {
        const editor = activeEditor();
        if (editor) void generateFromEditor(editor.view);
      },
    },
  ]);
}

/** The `code.generate` fallback: syntax-based, no language server needed. */
export function fallbackGenerateDefinition(view: EditorView): Promise<boolean> {
  return generateFromEditor(view);
}
