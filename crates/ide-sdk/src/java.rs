//! Installed JDKs: discovery, and the JDK/classpath choices that Maven, Gradle
//! and Java run configurations use.

use crate::home_dir;
use ide_language::LanguageServerConfig;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A JDK home and its feature version (8, 11, 17, 21…).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jdk {
    pub major: u32,
    pub home: PathBuf,
}

/// `JAVA_VERSION="21.0.3"` (or `"1.8.0_372"`) from a JDK's `release` file.
pub fn parse_release(release: &str) -> Option<u32> {
    let line = release.lines().find_map(|l| l.trim().strip_prefix("JAVA_VERSION="))?;
    let v = line.trim().trim_matches('"');
    let mut parts = v.split(['.', '_', '-', '+']);
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 { parts.next()?.parse().ok() } else { Some(first) }
}

/// The JDK at `home`, if it is one (a `release` file and `bin/java`).
pub fn jdk_at(home: &Path) -> Option<Jdk> {
    let release = std::fs::read_to_string(home.join("release")).ok()?;
    if !home.join("bin").join(crate::exe("java")).is_file() {
        return None;
    }
    Some(Jdk {
        major: parse_release(&release)?,
        home: dunce::canonicalize(home).unwrap_or_else(|_| home.to_path_buf()),
    })
}

fn children(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    out.sort();
    out
}

fn jdk_homes(root: &Path) -> impl Iterator<Item = PathBuf> {
    children(root)
        .into_iter()
        .flat_map(|dir| [dir.join("Contents/Home"), dir.join("libexec/openjdk.jdk/Contents/Home"), dir])
}

/// Vendor directories JDK installers use on Windows, under `Program Files`
/// and (per-user installs) `%LOCALAPPDATA%\Programs`.
const WINDOWS_VENDORS: &[&str] =
    &["Java", "Eclipse Adoptium", "Microsoft", "Zulu", "Amazon Corretto", "BellSoft", "Semeru", "OpenJDK"];

/// Folders the IDE downloads JDKs into ([`add_jdk_root`]).
static EXTRA_ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Scans the JDKs in `dir` (one per subfolder) too, e.g. where Download JDK puts them.
pub fn add_jdk_root(dir: &Path) {
    let mut roots = EXTRA_ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    if !roots.iter().any(|r| r == dir) {
        roots.push(dir.to_path_buf());
    }
}

/// Whether `home` lies in a folder registered with [`add_jdk_root`].
pub fn is_downloaded(home: &Path) -> bool {
    let roots = EXTRA_ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    roots.iter().any(|r| home.starts_with(r) || dunce::canonicalize(r).is_ok_and(|r| home.starts_with(r)))
}

/// JDK homes in the usual install locations (macOS `JavaVirtualMachines`,
/// `/usr/lib/jvm`, SDKMAN, mise, asdf, IDEA's `~/.jdks`, Homebrew, Windows vendor
/// directories, the IDE's downloads), `JAVA_HOME` first, then the `java` on `PATH`.
fn scan() -> Vec<Jdk> {
    let mut homes: Vec<PathBuf> = Vec::new();
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        homes.push(PathBuf::from(h));
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(java) = std::env::split_paths(&path).map(|d| d.join(crate::exe("java"))).find(|j| j.is_file()) {
        // Through symlinks (`/usr/bin/java`, Homebrew, SDKMAN `current`) to `<home>/bin/java`.
        if let Some(home) = dunce::canonicalize(java).ok().and_then(|j| j.parent()?.parent().map(Path::to_path_buf)) {
            homes.push(home);
        }
    }
    let home = home_dir();
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from("/Library/Java/JavaVirtualMachines"),
        PathBuf::from("/usr/lib/jvm"),
        PathBuf::from("/opt/homebrew/opt"),
        PathBuf::from("/usr/local/opt"),
    ];
    if let Some(h) = &home {
        roots.extend([h.join("Library/Java/JavaVirtualMachines"), h.join(".jdks"), h.join(".sdkman/candidates/java")]);
    }
    roots.extend(EXTRA_ROOTS.lock().unwrap_or_else(|e| e.into_inner()).iter().cloned());
    if cfg!(windows) {
        let bases = ["ProgramFiles", "ProgramW6432", "LOCALAPPDATA"].into_iter().filter_map(|v| {
            let dir = PathBuf::from(std::env::var_os(v)?);
            Some(if v == "LOCALAPPDATA" { dir.join("Programs") } else { dir })
        });
        for base in bases {
            roots.extend(WINDOWS_VENDORS.iter().map(|v| base.join(v)));
        }
    }
    for (i, root) in roots.iter().enumerate() {
        // `ProgramFiles` and `ProgramW6432` are usually the same directory.
        if roots[..i].contains(root) {
            continue;
        }
        homes.extend(jdk_homes(root));
    }
    // mise keeps a macOS JDK as its bundle (`<version>/Contents/Home`), asdf flattens it.
    for (dir, _) in ide_core::version_managers::installs(ide_core::version_managers::Tool::Java) {
        homes.extend([dir.join("Contents/Home"), dir]);
    }
    let mut out: Vec<Jdk> = Vec::new();
    for h in homes {
        if let Some(jdk) = jdk_at(&h)
            && !out.iter().any(|j| j.home == jdk.home)
        {
            out.push(jdk);
        }
    }
    out
}

static JDKS: Mutex<Option<Arc<[Jdk]>>> = Mutex::new(None);

/// Scans for installed JDKs again (Settings › Detect again).
pub fn rescan_jdks() -> Arc<[Jdk]> {
    let jdks: Arc<[Jdk]> = scan().into();
    *JDKS.lock().unwrap_or_else(|e| e.into_inner()) = Some(jdks.clone());
    jdks
}

/// Installed JDKs (scanned on first use).
pub fn installed_jdks() -> Arc<[Jdk]> {
    let mut slot = JDKS.lock().unwrap_or_else(|e| e.into_inner());
    match &*slot {
        Some(jdks) => jdks.clone(),
        None => {
            let jdks: Arc<[Jdk]> = scan().into();
            *slot = Some(jdks.clone());
            jdks
        }
    }
}

/// For a server with `max-java`: when the `JAVA_HOME` it would get (its env,
/// else the IDE's) is a newer JDK, points it at the newest of `jdks` that fits.
pub fn cap_java_home(cfg: &mut LanguageServerConfig, jdks: &[Jdk]) {
    let Some(max) = cfg.max_java else { return };
    let current = cfg.env.get("JAVA_HOME").cloned().or_else(|| std::env::var("JAVA_HOME").ok());
    if current.as_deref().and_then(|h| jdk_at(Path::new(h))).is_some_and(|j| j.major <= max) {
        return;
    }
    if let Some(jdk) = jdks.iter().filter(|j| j.major <= max).max_by_key(|j| j.major) {
        cfg.env.insert("JAVA_HOME".into(), jdk.home.to_string_lossy().into_owned());
    }
}

/// `found JDK 8, 17` / `no JDK was found`.
fn found_jdks(jdks: &[Jdk]) -> String {
    let mut found: Vec<u32> = jdks.iter().map(|j| j.major).collect();
    found.sort_unstable();
    found.dedup();
    if found.is_empty() {
        "no JDK was found".to_owned()
    } else {
        let list: Vec<String> = found.iter().map(u32::to_string).collect();
        format!("found JDK {}", list.join(", "))
    }
}

/// Why a module targeting Java `release` can't be built or run with the installed
/// JDKs (none is that new), or `None` when one is (or the release is unknown).
pub fn missing_jdk_message(jdks: &[Jdk], release: Option<u32>) -> Option<String> {
    let release = release?;
    if jdks.iter().any(|j| j.major >= release) {
        return None;
    }
    Some(format!(
        "This project targets Java {release}: it needs JDK {release} or newer ({}). Download JDK {release}, or \
         choose a Project JDK in Settings › Languages & Frameworks › Java.",
        found_jdks(jdks)
    ))
}

/// The `java` launcher of a JDK home.
pub fn java_executable(home: &Path) -> PathBuf {
    home.join("bin").join(crate::exe("java"))
}

/// The JDK to run a project targeting Java `release` with: that exact version,
/// else the oldest newer one, else the newest installed (`None`: any, newest).
pub fn pick_jdk(jdks: &[Jdk], release: Option<u32>) -> Option<&Jdk> {
    let newest = jdks.iter().max_by_key(|j| j.major);
    let Some(release) = release else { return newest };
    jdks.iter()
        .find(|j| j.major == release)
        .or_else(|| jdks.iter().filter(|j| j.major > release).min_by_key(|j| j.major))
        .or(newest)
}

/// Java version a Maven / Gradle module targets: `<java.version>`,
/// `<maven.compiler.release|source|target>` / `<release>`, Gradle toolchains
/// (`JavaLanguageVersion.of(21)`) or `sourceCompatibility`.
pub fn project_java_version(module_dir: &Path) -> Option<u32> {
    let read = |name: &str| std::fs::read_to_string(module_dir.join(name)).ok();
    if let Some(pom) = read("pom.xml") {
        for tag in
            ["java.version", "maven.compiler.release", "release", "maven.compiler.source", "maven.compiler.target"]
        {
            if let Some(v) = xml_value(&pom, tag).and_then(|v| parse_java_version(&v)) {
                return Some(v);
            }
        }
    }
    for name in ["build.gradle.kts", "build.gradle"] {
        let Some(text) = read(name) else { continue };
        if let Some(i) = text.find("JavaLanguageVersion.of(") {
            let rest = &text[i + "JavaLanguageVersion.of(".len()..];
            if let Some(v) = parse_java_version(rest.split(')').next().unwrap_or_default()) {
                return Some(v);
            }
        }
        for key in ["sourceCompatibility", "targetCompatibility"] {
            if let Some(i) = text.find(key) {
                let rest = text[i + key.len()..].trim_start_matches([' ', '=', '\t']);
                let token: String = rest
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != '\n')
                    .collect::<String>()
                    .trim_matches(['\'', '"'])
                    .replace("JavaVersion.VERSION_", "");
                if let Some(v) = parse_java_version(&token.replace('_', ".")) {
                    return Some(v);
                }
            }
        }
    }
    None
}

pub(crate) fn xml_value(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find('<')?;
    Some(text[start..start + end].trim().to_owned())
}

/// `21`, `"17"`, `1.8`, `'11'` → feature version.
fn parse_java_version(v: &str) -> Option<u32> {
    let v = v.trim().trim_matches(['"', '\'']);
    let mut parts = v.split('.');
    let first: u32 = parts.next()?.trim().parse().ok()?;
    if first == 1 { parts.next()?.trim().parse().ok() } else { Some(first) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_versions() {
        assert_eq!(parse_release("IMPLEMENTOR=\"x\"\nJAVA_VERSION=\"21.0.3\"\n"), Some(21));
        assert_eq!(parse_release("JAVA_VERSION=\"1.8.0_372\""), Some(8));
        assert_eq!(parse_release("JAVA_VERSION=\"25\""), Some(25));
        assert_eq!(parse_release("OTHER=1"), None);
    }

    #[test]
    fn missing_jdk_for_the_project_release() {
        let jdks = [Jdk { major: 17, home: PathBuf::from("/jdk17") }, Jdk { major: 8, home: PathBuf::from("/jdk8") }];
        assert_eq!(missing_jdk_message(&jdks, Some(17)), None);
        assert_eq!(missing_jdk_message(&jdks, Some(11)), None);
        assert_eq!(missing_jdk_message(&jdks, None), None);
        let msg = missing_jdk_message(&jdks, Some(21)).unwrap();
        assert!(msg.contains("needs JDK 21 or newer (found JDK 8, 17)") && msg.contains("Download JDK 21"), "{msg}");
        assert!(missing_jdk_message(&[], Some(8)).unwrap().contains("no JDK was found"));
    }

    #[test]
    fn downloaded_jdks_are_found() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("jdks/temurin-25.0.1");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(home.join("release"), "JAVA_VERSION=\"25.0.1\"\n").unwrap();
        std::fs::write(home.join("bin").join(crate::exe("java")), "").unwrap();
        // macOS bundles: <dir>/Contents/Home
        let bundle = tmp.path().join("jdks/temurin-21.0.5/Contents/Home");
        std::fs::create_dir_all(bundle.join("bin")).unwrap();
        std::fs::write(bundle.join("release"), "JAVA_VERSION=\"21.0.5\"\n").unwrap();
        std::fs::write(bundle.join("bin").join(crate::exe("java")), "").unwrap();
        add_jdk_root(&tmp.path().join("jdks"));
        let found = rescan_jdks();
        let home = dunce::canonicalize(&home).unwrap();
        let bundle = dunce::canonicalize(&bundle).unwrap();
        assert!(found.iter().any(|j| j.home == home && j.major == 25), "{found:?}");
        assert!(found.iter().any(|j| j.home == bundle && j.major == 21), "{found:?}");
        assert!(is_downloaded(&home) && !is_downloaded(Path::new("/elsewhere")));
    }

    #[test]
    fn picks_the_jdk_matching_the_project() {
        let jdks = [
            Jdk { major: 25, home: PathBuf::from("/jdk25") },
            Jdk { major: 21, home: PathBuf::from("/jdk21") },
            Jdk { major: 17, home: PathBuf::from("/jdk17") },
        ];
        assert_eq!(pick_jdk(&jdks, Some(21)).unwrap().major, 21);
        assert_eq!(pick_jdk(&jdks, Some(19)).unwrap().major, 21, "oldest newer one");
        assert_eq!(pick_jdk(&jdks, Some(26)).unwrap().major, 25, "newest as last resort");
        assert_eq!(pick_jdk(&jdks, None).unwrap().major, 25);
        assert!(pick_jdk(&[], Some(21)).is_none());
        assert_eq!(java_executable(Path::new("/jdk21")), Path::new("/jdk21/bin").join(crate::exe("java")));
    }

    #[test]
    fn project_java_versions() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert_eq!(project_java_version(dir), None);
        std::fs::write(dir.join("pom.xml"), "<properties>\n<java.version>21</java.version>\n</properties>").unwrap();
        assert_eq!(project_java_version(dir), Some(21));
        std::fs::write(dir.join("pom.xml"), "<maven.compiler.source>1.8</maven.compiler.source>").unwrap();
        assert_eq!(project_java_version(dir), Some(8));
        std::fs::remove_file(dir.join("pom.xml")).unwrap();
        std::fs::write(
            dir.join("build.gradle.kts"),
            "java {\n  toolchain {\n    languageVersion = JavaLanguageVersion.of(17)\n  }\n}",
        )
        .unwrap();
        assert_eq!(project_java_version(dir), Some(17));
        std::fs::write(dir.join("build.gradle.kts"), "java { sourceCompatibility = JavaVersion.VERSION_11 }").unwrap();
        assert_eq!(project_java_version(dir), Some(11));
        std::fs::write(dir.join("build.gradle.kts"), "sourceCompatibility = '1.8'").unwrap();
        assert_eq!(project_java_version(dir), Some(8));
    }
}
