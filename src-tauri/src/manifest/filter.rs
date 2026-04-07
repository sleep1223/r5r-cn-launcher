use crate::manifest::ManifestEntry;

/// User-generated files we never overwrite. Matches the official launcher's
/// `IsUserGeneratedContent` filter (`GameFileManager.cs:52-58`). Both forward
/// and back slashes are checked because the manifest can contain either.
pub fn is_user_generated(path: &str) -> bool {
    let p = path.to_ascii_lowercase().replace('/', "\\");
    p.contains("platform\\cfg\\user")
        || p.contains("platform\\screenshots")
        || p.contains("platform\\logs")
}

/// Whether this file matches one of the languages we want to install.
/// Files with an empty `language` are always required (base game).
///
/// `wanted` is the set of language codes (e.g. `["schinese"]`) the user
/// has chosen to install. The CN launcher hardcodes `schinese` for v1.
pub fn is_language_match(entry: &ManifestEntry, wanted: &[&str]) -> bool {
    if entry.language.is_empty() {
        return true;
    }
    wanted.iter().any(|w| w.eq_ignore_ascii_case(&entry.language))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_user_generated_paths() {
        assert!(is_user_generated(r"platform\cfg\user\autoexec.cfg"));
        assert!(is_user_generated(r"platform/cfg/user/autoexec.cfg"));
        assert!(is_user_generated(r"PLATFORM\Screenshots\foo.png"));
        assert!(is_user_generated(r"platform\logs\client.log"));
        assert!(!is_user_generated(r"platform\paks\client_default.bnk"));
        assert!(!is_user_generated(r"r5apex.exe"));
    }

    #[test]
    fn empty_language_always_matches() {
        let e = ManifestEntry {
            language: "".into(),
            ..Default::default()
        };
        assert!(is_language_match(&e, &["schinese"]));
        assert!(is_language_match(&e, &[]));
    }

    #[test]
    fn matches_chosen_language_only() {
        let e = ManifestEntry {
            language: "schinese".into(),
            ..Default::default()
        };
        assert!(is_language_match(&e, &["schinese"]));
        assert!(!is_language_match(&e, &["english"]));
    }
}
