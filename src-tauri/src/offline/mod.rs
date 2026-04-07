pub mod dir_import;
pub mod shape_detect;
pub mod zip_import;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "path", rename_all = "snake_case")]
pub enum OfflineSource {
    Directory(String),
    Zip(String),
}
