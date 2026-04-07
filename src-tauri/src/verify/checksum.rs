use crate::error::AppResult;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Compute the SHA-256 of a file as a lowercase hex string. Done on a blocking
/// thread because we don't want to hog the async runtime with disk IO.
pub async fn sha256_file(path: &Path) -> AppResult<String> {
    let path = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || -> AppResult<String> {
        use std::fs::File;
        use std::io::{BufReader, Read};
        let file = File::open(&path)?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    })
    .await
    .map_err(|e| crate::error::AppError::other(format!("hash task: {}", e)))??;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn known_sha256() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"hello").unwrap();
        let h = sha256_file(f.path()).await.unwrap();
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
