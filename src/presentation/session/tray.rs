//! The tray's view from the session the window uses: one process, one transport, one `Status`.

use super::{ServiceState, Session};
use crate::presentation::tray::TrayView;

impl Session {
    /// The icon, tooltip and menu. Until the service answers, the tray shows it as not running.
    pub fn tray(&self) -> TrayView {
        let status = self
            .data
            .status
            .as_ref()
            .filter(|_| self.service == ServiceState::Running);
        TrayView::of(status)
    }
}
