//! Project identity, copied from the web's `projectColor.ts`: a colour slot derived from the
//! name and IDEA's initials rule, so the title bar's avatar matches the webview byte for byte.

/// IDEA's `AvatarUtils.initials`: two camel-case capitals, else the first letters of the first
/// and last words, else one letter. `py-demo` → `PD`, `MyProject` → `MP`, `idea` → `I`.
pub fn initials(name: &str) -> String {
    let text = name.trim();
    let camel = camel_initials(text);
    if camel.chars().count() == 2 {
        return camel;
    }
    for separator in [' ', ',', '-', '_', '.', '`', '\'', '"'] {
        let words: Vec<&str> = text.split(separator).filter(|word| !word.is_empty()).collect();
        if words.len() >= 2 {
            let first = words[0].chars().next().unwrap_or('?');
            let last = words[words.len() - 1].chars().next().unwrap_or('?');
            return format!("{first}{last}").to_uppercase();
        }
    }
    if camel.is_empty() { "?".to_owned() } else { camel }
}

/// The first letter plus the next capital of the leading word: `MyProject` → `MP`.
fn camel_initials(text: &str) -> String {
    let word: String =
        text.split(|c: char| !c.is_alphanumeric()).find(|word| !word.is_empty()).unwrap_or_default().to_owned();
    let mut chars = word.chars();
    let Some(first) = chars.next() else { return String::new() };
    let capital = chars.find(|c| c.is_uppercase());
    match capital {
        Some(capital) => format!("{first}{capital}").to_uppercase(),
        None => first.to_uppercase().to_string(),
    }
}

/// Stable 1-based colour slot, exactly the hash the web uses (`h * 31 + code`, then `% 9`).
pub fn color_index(name: &str) -> usize {
    let mut hash: i32 = 0;
    for c in name.chars() {
        hash = hash.wrapping_mul(31).wrapping_add(c as i32);
    }
    (hash.unsigned_abs() as usize % 9) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_follow_idea() {
        assert_eq!(initials("MyProject"), "MP");
        assert_eq!(initials("py-demo"), "PD");
        assert_eq!(initials("idea"), "I");
        assert_eq!(initials("kwbasedb"), "K");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn colour_slot_is_stable_and_in_range() {
        for name in ["kwbasedb", "proj", "MyProject", "py-demo"] {
            let slot = color_index(name);
            assert!((1..=9).contains(&slot), "{name} -> {slot}");
        }
        assert_eq!(color_index("kwbasedb"), color_index("kwbasedb"));
    }
}
