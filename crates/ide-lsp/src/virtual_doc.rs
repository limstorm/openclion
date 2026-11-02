//! Virtual documents: read-only contents a language server provides for a
//! URI (`workspace/textDocumentContent`).

use percent_encoding::percent_decode_str;

/// `scheme` of a URI (`untitled` for `untitled:…`).
pub fn scheme(uri: &str) -> &str {
    uri.split_once(':').map(|(s, _)| s).unwrap_or("")
}

/// The URI's path segments (without query / fragment), percent-decoded.
fn segments(uri: &str) -> Vec<String> {
    let rest = uri.split_once(':').map(|(_, r)| r).unwrap_or(uri);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let rest = rest.strip_prefix("//").unwrap_or(rest);
    rest.split('/').filter(|s| !s.is_empty()).map(|s| percent_decode_str(s).decode_utf8_lossy().into_owned()).collect()
}

/// Editor tab title: the last path segment.
pub fn title(uri: &str) -> String {
    segments(uri).pop().unwrap_or_else(|| uri.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles() {
        assert_eq!(title("untitled:foo/bar.txt"), "bar.txt");
        assert_eq!(title("mocklib://deps/lib-1.0/Thing.mk"), "Thing.mk");
        assert_eq!(title("mocklib://deps/lib%20one/Thing.mk?rev=2"), "Thing.mk");
        assert_eq!(scheme("mocklib://deps/lib/Thing.mk"), "mocklib");
    }
}
