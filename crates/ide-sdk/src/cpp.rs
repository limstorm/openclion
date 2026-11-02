//! C/C++ toolchains (CLion's "compiler kits"): the compilers installed on the
//! machine with the debugger and build tools that go with them, and what a CMake
//! profile needs to select one.
//!
//! Detection only looks at the `PATH` (plus Settings › Tools overrides) and runs
//! `<compiler> --version` once per compiler, so it is cheap enough to call while
//! the Toolchains settings page is open.

use ide_core::settings::Settings;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One detected C/C++ toolchain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CompilerKit {
    /// Stable id: the C compiler's path.
    pub id: String,
    /// Display name, e.g. `Clang 18.1.3` or `GCC 13.3.0`.
    pub name: String,
    /// `clang` | `gcc` | `cc`.
    pub family: String,
    pub compiler: String,
    pub cxx_compiler: String,
    pub version: Option<String>,
    /// The compiler's own first `--version` line.
    pub description: String,
    /// `lldb-dap` / `lldb` for Clang, `gdb` for GCC, when installed.
    pub debugger: Option<String>,
    pub cmake: Option<String>,
    /// `ninja`, else `make`.
    pub build_tool: Option<String>,
}

struct Candidate {
    cc: &'static str,
    cxx: &'static str,
    family: &'static str,
    /// Debuggers that match this compiler family, most specific first.
    debuggers: &'static [&'static str],
}

const CANDIDATES: &[Candidate] = &[
    Candidate { cc: "clang", cxx: "clang++", family: "clang", debuggers: &["lldb-dap", "lldb"] },
    Candidate { cc: "gcc", cxx: "g++", family: "gcc", debuggers: &["gdb"] },
    // Last resort: the system aliases, which usually point at one of the above.
    Candidate { cc: "cc", cxx: "c++", family: "cc", debuggers: &["lldb-dap", "gdb"] },
];

/// The toolchains installed for the project at `root` (A C/C++ pair wins over a
/// duplicate reported through the `cc`/`c++` aliases).
pub fn detect(root: &Path, settings: &Settings) -> Vec<CompilerKit> {
    let search = ide_core::tool_path::search_path();
    let find = |command: &str| {
        crate::apply::tool_command(root, settings, command)
            .or_else(|| ide_core::tool_path::find_command(command, &search, root))
    };
    let mut kits: Vec<CompilerKit> = Vec::new();
    for candidate in CANDIDATES {
        let (Some(compiler), Some(cxx)) = (find(candidate.cc), find(candidate.cxx)) else { continue };
        // `cc` and `gcc` are usually the same binary through a symlink.
        let canonical = compiler.canonicalize().unwrap_or_else(|_| compiler.clone());
        if kits.iter().any(|kit| Path::new(&kit.compiler) == canonical) {
            continue;
        }
        let description = version_line(&compiler).unwrap_or_default();
        let version = parse_version(&description);
        let name = match &version {
            Some(v) => format!("{} {v}", title(candidate.family)),
            None => title(candidate.family).to_owned(),
        };
        kits.push(CompilerKit {
            id: canonical.to_string_lossy().into_owned(),
            name,
            family: candidate.family.to_owned(),
            compiler: canonical.to_string_lossy().into_owned(),
            cxx_compiler: cxx.canonicalize().unwrap_or(cxx).to_string_lossy().into_owned(),
            version,
            description,
            debugger: candidate.debuggers.iter().find_map(|d| find(d)).map(|p| p.to_string_lossy().into_owned()),
            cmake: find("cmake").map(|p| p.to_string_lossy().into_owned()),
            build_tool: find("ninja")
                .or_else(|| find("make"))
                .map(|p| p.to_string_lossy().into_owned())
                .or_else(|| settings.tool_path("ninja").map(str::to_owned)),
        });
    }
    kits
}

fn title(family: &str) -> &'static str {
    match family {
        "clang" => "Clang",
        "gcc" => "GCC",
        _ => "C/C++",
    }
}

/// The first line of `<compiler> --version`.
fn version_line(compiler: &Path) -> Option<String> {
    let output = Command::new(compiler).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.lines().next().map(|l| l.trim().to_owned()).filter(|l| !l.is_empty())
}

/// The first `N.N[.N]` in a version line (`Ubuntu clang version 18.1.3` → `18.1.3`).
pub fn parse_version(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut start = None;
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_digit() {
            if start.is_none() {
                start = Some(i);
            }
        } else if *b != b'.' && start.is_some() {
            let candidate = &line[start.unwrap()..i];
            if candidate.matches('.').count() >= 1 {
                return Some(candidate.to_owned());
            }
            start = None;
        }
    }
    start.map(|s| line[s..].trim_end_matches('.').to_owned()).filter(|v| v.contains('.'))
}

/// A path that exists, else `None` (the settings page shows what is usable).
pub fn existing(path: Option<PathBuf>) -> Option<String> {
    path.filter(|p| p.is_file()).map(|p| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_numbers_come_from_the_compiler_banner() {
        assert_eq!(parse_version("Ubuntu clang version 18.1.3 (1ubuntu1)"), Some("18.1.3".into()));
        assert_eq!(parse_version("gcc (Ubuntu 13.3.0-6ubuntu2~24.04) 13.3.0"), Some("13.3.0".into()));
        assert_eq!(parse_version("Apple clang version 15.0.0 (clang-1500.3.9.4)"), Some("15.0.0".into()));
        assert_eq!(parse_version("cc (GCC) 13.3.0"), Some("13.3.0".into()));
        assert_eq!(parse_version("no version here"), None);
    }
}
