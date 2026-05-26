pub mod fetch;
pub mod paths;
pub mod remote;
pub mod settings;

pub use remote::{Channel, DEFAULT_CHANNEL};
pub use settings::{
    LauncherSettings, PerChannelState, UpdateStrategy, DEFAULT_MIRROR_DOMAIN, OFFICIAL_DOMAIN,
};
