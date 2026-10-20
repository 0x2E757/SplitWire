//! The settings files: the strict `key=value` format with line numbers, the global and per-profile
//! settings, the service-owned index and state files, and merging changes into a file's text.

mod key_value_file;
mod profile_index;
mod profile_settings;
mod setting_value;
mod settings;
mod settings_merge;
mod state_file;

pub use key_value_file::{KeyValueEntry, KeyValueFile, SettingProblem};
pub use profile_index::ProfileIndex;
pub use profile_settings::ProfileSettings;
pub use setting_value::SettingValue;
pub use settings::Settings;
pub use settings_merge::{MergedFile, SettingChange, SettingsMerge, SettingsScope};
pub use state_file::StateFile;
