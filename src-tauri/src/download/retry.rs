use crate::error::{AppError, AppResult};
use reqwest::StatusCode;
use std::future::Future;
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff: Duration,
    pub dont_retry_status: &'static [u16],
}

impl RetryPolicy {
    pub const fn full_file() -> Self {
        Self {
            max_attempts: 15,
            backoff: Duration::from_secs(3),
            dont_retry_status: &[404],
        }
    }

    pub const fn chunk() -> Self {
        Self {
            max_attempts: 50,
            backoff: Duration::from_secs(3),
            dont_retry_status: &[404],
        }
    }

    pub async fn run<F, Fut, T>(&self, mut f: F) -> AppResult<T>
    where
        F: FnMut(u32) -> Fut,
        Fut: Future<Output = AppResult<T>>,
    {
        let mut last: Option<AppError> = None;
        for attempt in 1..=self.max_attempts {
            match f(attempt).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if let AppError::Http(msg) = &e {
                        if let Some(code) = extract_status_code(msg) {
                            if self.dont_retry_status.contains(&code) {
                                return Err(e);
                            }
                        }
                    }
                    if matches!(e, AppError::Cancelled) {
                        return Err(e);
                    }
                    last = Some(e);
                    if attempt < self.max_attempts {
                        tokio::time::sleep(self.backoff).await;
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| AppError::other("retry exhausted")))
    }
}

fn extract_status_code(msg: &str) -> Option<u16> {
    // Look for "HTTP NNN" or "status: NNN" patterns we use in our error
    // strings. Keep this loose — it's just a hint for the don't-retry list.
    for word in msg.split(|c: char| !c.is_ascii_digit()) {
        if word.len() == 3 {
            if let Ok(n) = word.parse::<u16>() {
                if (100..=599).contains(&n) {
                    return Some(n);
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
fn _ensure_status_imported() -> StatusCode {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn retries_until_success() {
        let p = RetryPolicy {
            max_attempts: 5,
            backoff: Duration::from_millis(1),
            dont_retry_status: &[404],
        };
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let r: AppResult<i32> = p
            .run(|attempt| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    if attempt < 3 {
                        Err(AppError::http("HTTP 500"))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;
        assert_eq!(r.unwrap(), 42);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_on_404() {
        let p = RetryPolicy {
            max_attempts: 5,
            backoff: Duration::from_millis(1),
            dont_retry_status: &[404],
        };
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();
        let r: AppResult<()> = p
            .run(|_| {
                let c = c.clone();
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Err(AppError::http("HTTP 404 Not Found"))
                }
            })
            .await;
        assert!(r.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
