pub mod api;
pub mod checker;
pub mod manager;
pub mod runner;

pub use api::{FormField, NotificationLevel, NotificationRequest, PanelWidget, PluginCommand, PluginContext, PluginResponse, SessionInfoData, SessionTarget};
pub use checker::{CheckResult, Diagnostic, Severity, check_plugin};
pub use manager::{PluginKeybind, PluginMeta, PluginType, discover_plugins, validate_icon_bytes};
pub use runner::{run_plugin, run_panel_plugin};
