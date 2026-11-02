/**
 * Settings › Build, Execution, Deployment › Toolchains: the executables the
 * C/C++ toolchain launches (CMake, the build tool, lldb-dap, clangd) and
 * whether clangd runs clang-tidy inspections — CLion's Clang-Tidy integration.
 *
 * Paths are stored in `tools.<command>.path`, which the backend applies to
 * language servers (clangd) and to run configurations (cmake, ctest, ninja).
 * clang-tidy is not launched by the IDE: clangd runs it, so the toggle edits
 * clangd's arguments instead.
 */
import { useEffect } from "react";
import { useSetting } from "@/features/settings/components/context";
import { tx } from "@/platform/i18n";
import { loadKits, useKits } from "./kits";
import { Row, Section } from "@/features/settings/components/Form";
import type { LanguageServerEntry } from "@/features/settings/model/schema";
import { Button, Checkbox, Input, T } from "@/ui";
import styles from "@/features/settings/pages/Workflow.module.css";

/** clangd's arguments as `languages/cpp.toml` declares them. */
const CLANGD_ARGS = [
  "--background-index",
  "--header-insertion=never",
  "--clang-tidy",
  "--compile-commands-dir=${compileCommands}",
];
const CLANG_TIDY = "--clang-tidy";

/** Executables the CMake toolchain launches, in the order CLion lists them. */
const TOOLS: ReadonlyArray<{ command: string; label: string; placeholder: string }> = [
  { command: "cmake", label: "CMake", placeholder: "Auto-detected on PATH" },
  { command: "ninja", label: "Build tool (Ninja)", placeholder: "Auto-detected on PATH" },
  { command: "make", label: "Build tool (Make)", placeholder: "Auto-detected on PATH" },
  { command: "lldb-dap", label: "Debugger (lldb-dap)", placeholder: "Auto-detected on PATH" },
  { command: "clangd", label: "clangd", placeholder: "Auto-detected on PATH" },
];

export function CppToolchain() {
  const [tools, setTools] = useSetting<Record<string, { path: string }>>("tools");
  const kits = useKits((s) => s.kits);
  const detected = useKits((s) => s.loaded);
  useEffect(() => {
    void loadKits();
  }, []);
  const [servers, setServers] = useSetting<Record<string, LanguageServerEntry>>("languageServers");
  const clangd = servers.clangd ?? {};
  const args = clangd.args ?? CLANGD_ARGS;
  const clangTidy = args.includes(CLANG_TIDY);

  const setPath = (command: string, path: string) => {
    const next = { ...tools };
    if (path.trim()) next[command] = { path };
    else delete next[command];
    setTools(next);
  };

  const setClangTidy = (enabled: boolean) => {
    // Checking restores the language definition's arguments; unchecking pins the
    // rest explicitly, so the server keeps them when the defaults change.
    const next = enabled ? CLANGD_ARGS : CLANGD_ARGS.filter((a) => a !== CLANG_TIDY);
    setServers({ ...servers, clangd: { ...clangd, args: [...next] } });
  };

  return (
    <div className={styles.settingsLayout}>
      <Section title="Toolchain">
        {TOOLS.map((tool) => (
          <Row
            key={tool.command}
            label={tool.label}
            htmlFor={`toolchain-${tool.command}`}
            className={styles.alignedRow}
          >
            <Input
              id={`toolchain-${tool.command}`}
              className={styles.wide}
              value={tools[tool.command]?.path ?? ""}
              placeholder={tool.placeholder}
              onChange={(e) => setPath(tool.command, e.target.value)}
            />
          </Row>
        ))}
        <div className={styles.note}>
          <T>
            Paths apply to clangd, the CMake build and the debugger. A project-scoped toolchain overrides the IDE-wide
            one for that project only.
          </T>
        </div>
      </Section>
      <Section title="Detected toolchains">
        {detected && kits.length === 0 ? (
          <div className={styles.note}>
            <T>No C/C++ compiler was found on PATH. Install clang or GCC and refresh.</T>
          </div>
        ) : (
          <div role="list" aria-label={tx("Detected toolchains")}>
            {kits.map((kit) => (
              <div key={kit.id} role="listitem" className={styles.note}>
                <strong>{kit.name}</strong>
                {kit.debugger ? ` · ${kit.debugger}` : ""}
                {kit.buildTool ? ` · ${kit.buildTool}` : ""}
                <div>
                  {kit.compiler} · {kit.cxxCompiler}
                </div>
              </div>
            ))}
          </div>
        )}
        <Button onClick={() => void loadKits(true)}>
          <T>Refresh</T>
        </Button>
      </Section>
      <Section title="Clang-Tidy">
        <Row label="Run clang-tidy inspections">
          <Checkbox checked={clangTidy} onChange={setClangTidy} label="Report clang-tidy warnings in the editor" />
        </Row>
        <div className={styles.note}>
          <T>
            clangd runs clang-tidy in the background and reports its findings as inspections. Projects configure the
            checks with a .clang-tidy file.
          </T>
        </div>
      </Section>
    </div>
  );
}
