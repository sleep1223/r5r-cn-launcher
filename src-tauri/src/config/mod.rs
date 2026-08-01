pub mod fetch;
pub mod local_version;
pub mod paths;
pub mod remote;
pub mod settings;

pub use remote::{Channel, RemoteConfig, DEFAULT_CHANNEL};
pub use settings::{
    LauncherSettings, PerChannelState, UpdateStrategy, DEFAULT_MIRROR_DOMAIN, OFFICIAL_DOMAIN,
};
