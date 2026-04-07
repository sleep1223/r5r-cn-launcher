pub mod fetch;
pub mod paths;
pub mod remote;
pub mod settings;

pub use remote::{Channel, RemoteConfig};
pub use settings::{LauncherSettings, PerChannelState};
