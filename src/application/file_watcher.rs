use std::sync::{Arc, Mutex};

use super::Coordinator;

/// Every second: hand edits of the settings files or the lists are picked up, by
/// comparing the files' sizes and times with the last look.
pub struct FileWatcher {
    coordinator: Arc<Coordinator>,
    last: Mutex<Option<(u64, u64)>>,
}

impl FileWatcher {
    pub fn new(coordinator: Arc<Coordinator>) -> Self {
        Self {
            coordinator,
            last: Mutex::new(None),
        }
    }

    pub fn tick(&self) {
        let (settings, lists) = self.coordinator.file_signature();
        let previous = self.last.lock().unwrap().replace((settings, lists));
        let Some((old_settings, old_lists)) = previous else {
            return;
        };
        if settings != old_settings {
            self.coordinator.reload_settings();
        }
        if lists != old_lists {
            self.coordinator.reload_lists();
        }
    }
}
