/**
 * Method declarations on a source line, named the way debug adapters expect them in
 * function breakpoints. Text-based (no PSI): good for ordinary declarations, which is
 * what IDEA's gutter toggles act on.
 */

export interface MethodTarget {
  /** Function breakpoint name for the language's adapter. */
  name: string;
  /** Short label (`Order.total()`). */
  label: string;
}

const CONTROL = new Set([
  "if",
  "for",
  "while",
  "switch",
  "catch",
  "return",
  "new",
  "throw",
  "else",
  "do",
  "try",
  "synchronized",
  "assert",
  "super",
  "this",
]);

function goMethod(lines: readonly string[], line: number): MethodTarget | null {
  const m = /^\s*func\s+(?:\(\s*\w*\s*(\*?)\s*([A-Za-z_]\w*)(?:\[[^\]]*\])?\s*\)\s*)?([A-Za-z_]\w*)\s*[[(]/.exec(
    lines[line] ?? "",
  );
  if (!m) return null;
  const pkg = lines.map((l) => /^\s*package\s+(\w+)/.exec(l)?.[1]).find(Boolean) ?? "main";
  const [, star, recv, fn] = m;
  const name = recv ? (star ? `${pkg}.(*${recv}).${fn}` : `${pkg}.${recv}.${fn}`) : `${pkg}.${fn}`;
  return { name, label: recv ? `${recv}.${fn}()` : `${fn}()` };
}

function simple(re: RegExp, lines: readonly string[], line: number): MethodTarget | null {
  const m = re.exec(lines[line] ?? "");
  return m ? { name: m[1]!, label: `${m[1]}()` } : null;
}

const C_FUNCTION =
  /^\s*(?:(?:static|inline|extern|virtual|constexpr|const|unsigned|signed|struct|enum)\s+)*[A-Za-z_][\w:<>,*&\s]*?[\s*&]([A-Za-z_~][\w:~]*)\s*\([^;]*$/;

/** The method declared on `line` (0-based), named for `language`'s debug adapter. */
export function methodAt(language: string, text: string | readonly string[], line: number): MethodTarget | null {
  const lines = typeof text === "string" ? text.split("\n") : text;
  switch (language) {
    case "python":
      return simple(/^\s*(?:async\s+)?def\s+([A-Za-z_]\w*)\s*\(/, lines, line);
    case "go":
      return goMethod(lines, line);
    case "rust":
      return simple(
        /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe|extern\s+"[^"]*")\s+)*fn\s+([A-Za-z_]\w*)/,
        lines,
        line,
      );
    case "javascript":
    case "typescript":
    case "javascriptreact":
    case "typescriptreact":
      return (
        simple(/^\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?function\*?\s+([A-Za-z_$][\w$]*)/, lines, line) ??
        simple(
          /^\s*(?:(?:public|private|protected|static|async|get|set)\s+)*([A-Za-z_$][\w$]*)\s*\([^)]*\)\s*(?::[^{]*)?\{/,
          lines,
          line,
        )
      );
    case "c":
    case "cpp":
    case "objective-c":
    case "objective-cpp": {
      const t = simple(C_FUNCTION, lines, line);
      return t && !CONTROL.has(t.name) ? t : null;
    }
    case "swift":
      return simple(/^\s*(?:[\w@]+\s+)*func\s+([A-Za-z_]\w*)/, lines, line);
    default:
      return null;
  }
}
