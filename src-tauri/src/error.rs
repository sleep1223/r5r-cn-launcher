use serde::{Serialize, Serializer};
use std::fmt;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Http(String),
    Manifest(String),
    Verification { path: String, expected: String, actual: String },
    Settings(String),
    InvalidPath(String),
    Cancelled,
    NotFound(String),
    Other(String),
}

impl AppError {
    pub fn other<S: Into<String>>(s: S) -> Self {
        AppError::Other(s.into())
    }
    pub fn settings<S: Into<String>>(s: S) -> Self {
        AppError::Settings(s.into())
    }
    pub fn http<S: Into<String>>(s: S) -> Self {
        AppError::Http(s.into())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO 错误: {}", e),
            AppError::Http(s) => write!(f, "网络错误: {}", s),
            AppError::Manifest(s) => write!(f, "清单错误: {}", s),
            AppError::Verification { path, .. } => write!(f, "校验失败: {}", path),
            AppError::Settings(s) => write!(f, "设置错误: {}", s),
            AppError::InvalidPath(s) => write!(f, "路径无效: {}", s),
            AppError::Cancelled => write!(f, "已取消"),
            AppError::NotFound(s) => write!(f, "未找到: {}", s),
            AppError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(format!("JSON 解析: {}", e))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

/// IPC-friendly serialization: returns `{ kind, message }` so the React side
/// can branch on `kind` for special-case UI (e.g. show a retry button on
/// `Cancelled`, but a settings link on `Settings`).
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let kind = match self {
            AppError::Io(_) => "io",
            AppError::Http(_) => "http",
            AppError::Manifest(_) => "manifest",
            AppError::Verification { .. } => "verification",
            AppError::Settings(_) => "settings",
            AppError::InvalidPath(_) => "invalid_path",
            AppError::Cancelled => "cancelled",
            AppError::NotFound(_) => "not_found",
            AppError::Other(_) => "other",
        };
        let mut st = s.serialize_struct("AppError", 2)?;
        st.serialize_field("kind", kind)?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}
