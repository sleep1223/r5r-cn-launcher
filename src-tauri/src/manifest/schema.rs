//! Wire-compatible mirror of the official R5Reloaded `GameManifest` schema.
//! These structs deserialize an unmodified `checksums.json` produced by the
//! official `patch_creator` tool.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameManifest {
    #[serde(default)]
    pub game_version: String,
    #[serde(default)]
    pub blog_slug: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub files: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestEntry {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub checksum: String, // sha256 hex, lowercase
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub parts: Vec<FileChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileChunk {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub checksum: String, // sha256 hex
    #[serde(default)]
    pub size: u64,
}
