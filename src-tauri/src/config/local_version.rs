use std::path::Path;

/// Read the installed game's build marker and convert it to the version shape
/// used by checksums.json. For example, `R5R-v2.6.42` becomes
/// `2.6.42-live` when the remote manifest is `2.6.51-live`.
pub async fn read_build_version(install_dir: &Path, remote_version: &str) -> Option<String> {
    let contents = tokio::fs::read_to_string(install_dir.join("build.txt"))
        .await
        .ok()?;
    parse_build_version(&contents, remote_version)
}

pub fn parse_build_version(contents: &str, remote_version: &str) -> Option<String> {
    let raw = contents.trim().trim_start_matches('\u{feff}');
    const PREFIX: &str = "R5R-v";
    let prefix = raw.get(..PREFIX.len())?;
    if !prefix.eq_ignore_ascii_case(PREFIX) {
        return None;
    }

    let build = raw.get(PREFIX.len()..)?.trim();
    if build.is_empty() || build.chars().any(char::is_whitespace) {
        return None;
    }
    if build.contains('-') {
        return Some(build.to_string());
    }

    let suffix = remote_version
        .find('-')
        .map(|index| &remote_version[index..])
        .unwrap_or_default();
    Some(format!("{}{}", build, suffix))
}

/// Convert the dashboard's display version (`v2.6.51`) into the manifest and
/// patch version shape (`2.6.51-live`), borrowing the channel suffix from the
/// manifest returned for that release.
pub fn normalize_community_version(
    community_version: &str,
    manifest_version: &str,
) -> Option<String> {
    let trimmed = community_version.trim();
    let version = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed)
        .trim();
    if version.is_empty() {
        return None;
    }
    if version.contains('-') {
        return Some(version.to_string());
    }
    let suffix = manifest_version
        .find('-')
        .map(|index| &manifest_version[index..])
        .unwrap_or_default();
    Some(format!("{}{}", version, suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_live_build_version() {
        assert_eq!(
            parse_build_version("R5R-v2.6.42\r\n", "2.6.51-live"),
            Some("2.6.42-live".to_string())
        );
    }

    #[test]
    fn preserves_an_existing_channel_suffix() {
        assert_eq!(
            parse_build_version("r5r-v2.6.42-live", "2.6.51-live"),
            Some("2.6.42-live".to_string())
        );
    }

    #[test]
    fn rejects_an_unrelated_build_marker() {
        assert_eq!(parse_build_version("2.6.42", "2.6.51-live"), None);
    }

    #[test]
    fn normalizes_dashboard_version_for_patch_matching() {
        assert_eq!(
            normalize_community_version("v2.6.51", "2.6.51-live"),
            Some("2.6.51-live".to_string())
        );
    }
}
