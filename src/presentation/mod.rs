//! The window's view models: pure, built from IPC data, tested. The platform renders
//! them as system XAML and maps events back to `Command`s; the `Session` turns commands into
//! requests over the `EngineTransport` and keeps what the engine answered.

mod alerts;
mod command;
mod entry_check;
mod format;
mod import_batch;
mod local_time;
mod log_file;
mod nav;
mod page;
pub mod pages;
mod profile_menu;
mod profile_state;
pub mod session;
mod settings_edit;
pub mod showcase;
mod step_table;
mod traffic_chart;
pub mod transport;
pub mod tray;
mod width_class;
mod window_settings;

#[cfg(test)]
mod samples;

pub use alerts::{AlertList, AlertView, BellFlyout};
pub use command::{
    Command, Effect, InputId, LevelFilter, ListsMode, LogSource, PickedFile, ProfileFilter,
};
pub use entry_check::{EntryCheck, EntryVerdict};
pub use format::Format;
pub use import_batch::{ImportBatch, ReviewFile, UnreadFile};
pub use local_time::LocalTime;
pub use log_file::LogFile;
pub use nav::{NavItem, NavLead, NavSource, NavValue, NavView, Tone};
pub use page::Page;
pub use profile_menu::{MenuEntry, ProfileMenu};
pub use profile_state::{Dot, Pill, StateText};
pub use session::{Folders, FrameView, PageView, Session};
pub use settings_edit::{ApplyBarView, SettingsEdit};
pub use step_table::{StepRow, StepTableView};
pub use traffic_chart::{ChartSeries, ChartView, TrafficChart};
pub use width_class::WidthClass;
pub use window_settings::{Placement, ThemeChoice, WindowSettings};
