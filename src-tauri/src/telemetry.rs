use crate::config::LauncherSettings;
use reqwest::Client;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct LauncherOpenPayload<'a> {
    installation_id: &'a str,
    launcher_version: &'a str,
    platform: &'a str,
    arch: &'a str,
}

fn should_report(settings: &LauncherSettings) -> bool {
    !cfg!(debug_assertions) && settings.usage_reporting_enabled
}

pub fn usage_open_url(config_url: &str) -> Option<String> {
    let mut url = url::Url::parse(config_url).ok()?;
    let path = url.path().trim_end_matches('/');
    let prefix = path.strip_suffix("/launcher/config")?;
    url.set_path(&format!("{prefix}/launcher/usage/open"));
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

pub async fn report_first_open(client: Client, settings: LauncherSettings) {
    if !should_report(&settings) {
        return;
    }
    if uuid::Uuid::parse_str(&settings.installation_id)
        .ok()
        .filter(|id| id.get_version_num() == 4)
        .is_none()
    {
        tracing::warn!(target: "usage", "跳过匿名使用统计：installation_id 无效");
        return;
    }
    let Some(endpoint) = usage_open_url(&settings.dashboard_api_url) else {
        tracing::warn!(target: "usage", "跳过匿名使用统计：数据面板地址无效");
        return;
    };
    let payload = LauncherOpenPayload {
        installation_id: &settings.installation_id,
        launcher_version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    };
    match client.post(endpoint).json(&payload).send().await {
        Ok(response) if response.status().is_success() => {
            tracing::debug!(target: "usage", "匿名首次使用统计上报成功");
        }
        Ok(response) => {
            tracing::warn!(target: "usage", "匿名使用统计上报失败：HTTP {}", response.status());
        }
        Err(error) => {
            tracing::warn!(target: "usage", "匿名使用统计上报失败：{}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_usage_endpoint_from_dashboard_config_url() {
        assert_eq!(
            usage_open_url("https://r5.sleep0.de/api/v1/r5/launcher/config"),
            Some("https://r5.sleep0.de/api/v1/r5/launcher/usage/open".to_string())
        );
        assert_eq!(usage_open_url("https://example.com/other"), None);
    }

    #[test]
    fn debug_builds_and_disabled_settings_do_not_report() {
        let mut settings = LauncherSettings::default();
        settings.usage_reporting_enabled = false;
        assert!(!should_report(&settings));
        if cfg!(debug_assertions) {
            settings.usage_reporting_enabled = true;
            assert!(!should_report(&settings));
        }
    }

    #[test]
    fn payload_contains_only_anonymous_installation_fields() {
        let payload = LauncherOpenPayload {
            installation_id: "6315aa5c-cb67-457f-8c70-8ac64215678e",
            launcher_version: "1.2.3",
            platform: "windows",
            arch: "x86_64",
        };
        let value = serde_json::to_value(payload).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 4);
        assert_eq!(value["platform"], "windows");
    }
}
