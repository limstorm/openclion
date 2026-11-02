//! Release metadata: GitHub releases, HashiCorp releases, Eclipse JDT LS
//! milestones. Parsing is pure so it is testable without network.

use crate::platform::{AssetRule, Platform, select_asset};
use ide_core::error::ErrorKind;
use ide_core::{IdeError, IdeResult};
use serde::Deserialize;

/// One downloadable file chosen for this platform.
#[derive(Clone, Debug, PartialEq)]
pub struct Download {
    pub version: String,
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
    /// Lower-case hex SHA-256 when the publisher provides one.
    pub sha256: Option<String>,
    /// URL of a checksum file to fetch when `sha256` is not inline.
    pub checksum_url: Option<String>,
}

fn proto(msg: impl Into<String>) -> IdeError {
    IdeError::new(ErrorKind::Protocol, msg)
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: Option<u64>,
    /// `sha256:<hex>` on releases published since mid-2025.
    #[serde(default)]
    digest: Option<String>,
}

pub fn github_latest_url(repo: &str) -> String {
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

/// Picks the platform asset from a GitHub `releases/latest` response. Links
/// in the release notes count as assets too (some projects host binaries on
/// a CDN and only link them).
pub fn github_pick(json: &str, rules: &[AssetRule], platform: Platform) -> IdeResult<Download> {
    let rel: GhRelease = serde_json::from_str(json).map_err(|e| proto(format!("unexpected GitHub response: {e}")))?;
    let version = rel.tag_name.trim_start_matches('v').to_string();
    let names: Vec<&str> = rel.assets.iter().map(|a| a.name.as_str()).collect();
    if let Some(name) = select_asset(&names, rules, platform) {
        let a = rel.assets.iter().find(|a| a.name == name).expect("selected from list");
        let sha256 = a.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).map(str::to_ascii_lowercase);
        let checksum_url = rel
            .assets
            .iter()
            .find(|c| c.name == format!("{name}.sha256") || c.name == format!("{name}.sha256sum"))
            .map(|c| c.browser_download_url.clone());
        return Ok(Download {
            version,
            name: a.name.clone(),
            url: a.browser_download_url.clone(),
            size: a.size,
            sha256,
            checksum_url,
        });
    }
    let links = extract_links(rel.body.as_deref().unwrap_or(""));
    let link_names: Vec<&str> = links.iter().map(|u| file_name(u)).collect();
    if let Some(name) = select_asset(&link_names, rules, platform) {
        let url = links.iter().find(|u| file_name(u) == name).expect("selected from list").clone();
        let checksum_url = links.iter().find(|u| *u == &format!("{url}.sha256")).cloned();
        return Ok(Download { version, name: name.to_string(), url, size: None, sha256: None, checksum_url });
    }
    Err(IdeError::unavailable(format!("release {} has no build for this platform", rel.tag_name)))
}

fn file_name(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/').next().unwrap_or(path)
}

/// `http(s)://…` tokens in markdown / plain text.
fn extract_links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("https://").or_else(|| rest.find("http://")) {
        let tail = &rest[i..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '(' | '"' | '\'' | '<' | '>' | ']' | '[' | '`'))
            .unwrap_or(tail.len());
        let url = tail[..end].trim_end_matches(['.', ',', ';']);
        if !out.iter().any(|u| u == url) {
            out.push(url.to_string());
        }
        rest = &tail[end..];
    }
    out
}

#[derive(Deserialize)]
struct HcRelease {
    version: String,
    #[serde(default)]
    builds: Vec<HcBuild>,
    #[serde(default)]
    url_shasums: Option<String>,
}

#[derive(Deserialize)]
struct HcBuild {
    url: String,
}

pub fn hashicorp_latest_url(product: &str) -> String {
    format!("https://api.releases.hashicorp.com/v1/releases/{product}/latest")
}

pub fn hashicorp_pick(json: &str, rules: &[AssetRule], platform: Platform) -> IdeResult<Download> {
    let rel: HcRelease =
        serde_json::from_str(json).map_err(|e| proto(format!("unexpected HashiCorp response: {e}")))?;
    let names: Vec<&str> = rel.builds.iter().map(|b| file_name(&b.url)).collect();
    let name = select_asset(&names, rules, platform)
        .ok_or_else(|| IdeError::unavailable(format!("release {} has no build for this platform", rel.version)))?;
    let build = rel.builds.iter().find(|b| file_name(&b.url) == name).expect("selected from list");
    Ok(Download {
        version: rel.version.clone(),
        name: name.to_string(),
        url: build.url.clone(),
        size: None,
        sha256: None,
        checksum_url: rel.url_shasums,
    })
}

#[derive(Deserialize)]
struct VsxExtension {
    version: String,
    #[serde(default)]
    downloads: std::collections::BTreeMap<String, String>,
    files: VsxFiles,
}

#[derive(Deserialize)]
struct VsxFiles {
    download: String,
    #[serde(default)]
    sha256: Option<String>,
}

pub fn openvsx_latest_url(namespace: &str, extension: &str) -> String {
    format!("https://open-vsx.org/api/{namespace}/{extension}/latest")
}

/// Select platform metadata when the universal package does not match the asset rules.
pub fn openvsx_platform_url(json: &str, rules: &[AssetRule], platform: Platform) -> IdeResult<Option<String>> {
    let ext: VsxExtension =
        serde_json::from_str(json).map_err(|e| proto(format!("unexpected Open VSX response: {e}")))?;
    if select_asset(&[file_name(&ext.files.download)], rules, platform).is_some() {
        return Ok(None);
    }
    let names: Vec<&str> = ext.downloads.values().map(|url| file_name(url)).collect();
    let name = select_asset(&names, rules, platform)
        .ok_or_else(|| IdeError::unavailable("No Open VSX package for this platform"))?;
    let url = ext.downloads.values().find(|url| file_name(url) == name).expect("selected download");
    let (metadata, _) = url.rsplit_once("/file/").ok_or_else(|| proto("unexpected Open VSX download URL"))?;
    Ok(Some(metadata.to_string()))
}

/// The `.vsix` of an Open VSX `…/latest` response.
pub fn openvsx_pick(json: &str, rules: &[AssetRule], platform: Platform) -> IdeResult<Download> {
    let ext: VsxExtension =
        serde_json::from_str(json).map_err(|e| proto(format!("unexpected Open VSX response: {e}")))?;
    let name = file_name(&ext.files.download);
    if select_asset(&[name], rules, platform).is_none() {
        return Err(IdeError::unavailable(format!("{name} is not a known package for this platform")));
    }
    Ok(Download {
        version: ext.version.clone(),
        name: name.to_string(),
        url: ext.files.download.clone(),
        size: None,
        sha256: None,
        checksum_url: ext.files.sha256,
    })
}

/// Finds the hash for `file_name` in a checksum file: either a bare hex digest
/// or `sha256sum` lines (`<hex>  <name>`).
pub fn parse_checksum(text: &str, file_name: &str) -> Option<String> {
    let is_hex = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty()).peekable();
    let first = *lines.peek()?;
    if is_hex(first) {
        return Some(first.to_ascii_lowercase());
    }
    for line in lines {
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(name)) = (parts.next(), parts.next()) else { continue };
        if is_hex(hash) && name.trim_start_matches('*') == file_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

pub fn sha256_file(path: &std::path::Path) -> IdeResult<String> {
    use sha2::Digest;
    let mut f = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    std::io::copy(&mut f, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{Arch, Os};
    use crate::registry::find;

    const MAC_ARM: Platform = Platform { os: Os::Mac, arch: Arch::Arm64 };
    const LINUX: Platform = Platform { os: Os::Linux, arch: Arch::X64 };

    #[test]
    fn github_asset_with_digest() {
        let json = r#"{"tag_name":"v3.13.5","body":"notes","assets":[
            {"name":"lua-language-server-3.13.5-darwin-arm64.tar.gz","browser_download_url":"https://x/a.tgz","size":123,"digest":"sha256:ABCDEF"},
            {"name":"lua-language-server-3.13.5-linux-x64.tar.gz","browser_download_url":"https://x/b.tgz","size":456,"digest":null}
        ]}"#;
        let rules = find("lua-language-server").unwrap().asset_rules();
        let d = github_pick(json, rules, MAC_ARM).unwrap();
        assert_eq!(d.version, "3.13.5");
        assert_eq!(d.url, "https://x/a.tgz");
        assert_eq!(d.size, Some(123));
        assert_eq!(d.sha256.as_deref(), Some("abcdef"));
        let d = github_pick(json, rules, LINUX).unwrap();
        assert_eq!(d.sha256, None);
        let win = Platform { os: Os::Windows, arch: Arch::X64 };
        assert_eq!(github_pick(json, rules, win).unwrap_err().kind, ErrorKind::Unavailable);
    }

    #[test]
    fn github_sibling_checksum_asset() {
        let json = r#"{"tag_name":"2024-12-18","assets":[
            {"name":"marksman-macos","browser_download_url":"https://x/m","size":1},
            {"name":"marksman-macos.sha256","browser_download_url":"https://x/m.sha256","size":1}
        ]}"#;
        let d = github_pick(json, find("marksman").unwrap().asset_rules(), MAC_ARM).unwrap();
        assert_eq!(d.name, "marksman-macos");
        assert_eq!(d.checksum_url.as_deref(), Some("https://x/m.sha256"));
    }

    #[test]
    fn hashicorp() {
        let json = r#"{"version":"0.36.4","url_shasums":"https://r/terraform-ls_0.36.4_SHA256SUMS","builds":[
            {"arch":"arm64","os":"darwin","url":"https://r/terraform-ls_0.36.4_darwin_arm64.zip"},
            {"arch":"amd64","os":"linux","url":"https://r/terraform-ls_0.36.4_linux_amd64.zip"}]}"#;
        let d = hashicorp_pick(json, find("terraform-ls").unwrap().asset_rules(), MAC_ARM).unwrap();
        assert_eq!(d.name, "terraform-ls_0.36.4_darwin_arm64.zip");
        assert_eq!(d.checksum_url.as_deref(), Some("https://r/terraform-ls_0.36.4_SHA256SUMS"));
    }

    #[test]
    fn openvsx() {
        let json = r#"{"version":"0.3.8","files":{
            "download":"https://open-vsx.org/api/rogalmic/bash-debug/0.3.8/file/rogalmic.bash-debug-0.3.8.vsix",
            "sha256":"https://open-vsx.org/api/rogalmic/bash-debug/0.3.8/file/rogalmic.bash-debug-0.3.8.sha256"}}"#;
        let rules = find("bash-debug").unwrap().asset_rules();
        let d = openvsx_pick(json, rules, MAC_ARM).unwrap();
        assert_eq!(d.version, "0.3.8");
        assert_eq!(d.name, "rogalmic.bash-debug-0.3.8.vsix");
        assert!(d.checksum_url.unwrap().ends_with(".sha256"));
        let other = json.replace("rogalmic.bash-debug-0.3.8.vsix", "something-else.zip");
        assert!(openvsx_pick(&other, rules, LINUX).is_err());
    }

    #[test]
    fn openvsx_rust_analyzer_platform_packages() {
        use crate::platform::{Arch, Os};
        let crate::registry::Method::Release { assets, .. } = find("rust-analyzer").unwrap().fallback.unwrap() else {
            panic!("expected release fallback")
        };
        let targets = [
            (Os::Mac, Arch::Arm64, "darwin-arm64"),
            (Os::Mac, Arch::X64, "darwin-x64"),
            (Os::Linux, Arch::Arm64, "linux-arm64"),
            (Os::Linux, Arch::X64, "linux-x64"),
            (Os::Windows, Arch::Arm64, "win32-arm64"),
            (Os::Windows, Arch::X64, "win32-x64"),
        ];
        let base = "https://open-vsx.org/api/rust-lang/rust-analyzer";
        let downloads: serde_json::Map<String, serde_json::Value> = targets
            .iter()
            .map(|(_, _, target)| {
                (
                    target.to_string(),
                    format!("{base}/{target}/1.0.0/file/rust-lang.rust-analyzer-1.0.0@{target}.vsix").into(),
                )
            })
            .collect();
        let universal = serde_json::json!({
            "version": "1.0.0", "downloads": downloads,
            "files": {"download": format!("{base}/1.0.0/file/rust-lang.rust-analyzer-1.0.0.vsix")}
        })
        .to_string();
        for (os, arch, target) in targets {
            let platform = Platform { os, arch };
            assert_eq!(
                openvsx_platform_url(&universal, assets, platform).unwrap(),
                Some(format!("{base}/{target}/1.0.0"))
            );
            let json = serde_json::json!({
                "version": "1.0.0",
                "files": {
                    "download": downloads[target],
                    "sha256": format!("{base}/{target}/1.0.0/file/rust-lang.rust-analyzer-1.0.0@{target}.sha256")
                }
            })
            .to_string();
            let download = openvsx_pick(&json, assets, platform).unwrap();
            assert!(download.name.ends_with(&format!("@{target}.vsix")));
            assert!(download.checksum_url.unwrap().ends_with(&format!("@{target}.sha256")));
        }
        assert!(openvsx_platform_url(&universal.replace("downloads", "unused"), assets, MAC_ARM).is_err());
    }

    #[test]
    fn checksums() {
        let h = "a".repeat(64);
        assert_eq!(parse_checksum(&format!("{h}\n"), "x"), Some(h.clone()));
        let sums = format!("{}  other.zip\n{}  *wanted.zip\n", "b".repeat(64), "C".repeat(64));
        assert_eq!(parse_checksum(&sums, "wanted.zip"), Some("c".repeat(64)));
        assert_eq!(parse_checksum(&sums, "missing.zip"), None);
    }

    #[test]
    fn sha256_of_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"abc").unwrap();
        assert_eq!(
            sha256_file(tmp.path()).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
