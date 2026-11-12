// SPDX-License-Identifier: GPL-3.0-or-later

//! Syncs nobody asks for while Beacon stays open (asked by the user
//! 2026-11-12): every 15 minutes, and when the connection comes back after
//! it was lost, so changes made on other devices arrive without a refresh,
//! a pause or a restart. Both are quiet when they fail, as every sync not
//! asked for is, unless the account needs attention.

use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::SyncMsg;
use crate::app::{App, AppMsg};

/// How often a sync runs while Beacon is open.
pub(in crate::app) const EVERY: Duration = Duration::from_secs(15 * 60);

/// Whether the network was there, to tell when it comes back.
#[derive(Debug)]
pub(in crate::app) struct Connectivity {
    online: bool,
}

impl Connectivity {
    pub(in crate::app) fn new(online: bool) -> Self {
        Self { online }
    }

    /// The network is `online` now: whether it has just come back.
    pub(in crate::app) fn update(&mut self, online: bool) -> bool {
        let back = online && !self.online;
        self.online = online;
        back
    }
}

impl Default for Connectivity {
    fn default() -> Self {
        Self::new(gio::NetworkMonitor::default().is_network_available())
    }
}

impl App {
    /// Sync every [`EVERY`] from now on.
    pub(in crate::app) fn sync_periodically(&self) {
        let sender = self.sender.clone();
        glib::timeout_add_local(EVERY, move || {
            sender.input(AppMsg::Sync(SyncMsg::Periodic));
            glib::ControlFlow::Continue
        });
    }

    /// The periodic sync is due: only with a network to reach the server.
    pub(super) fn periodic_sync(&mut self) {
        if gio::NetworkMonitor::default().is_network_available() {
            self.request_sync(false);
        }
    }

    /// The network changed: sync if it has just come back.
    pub(in crate::app) fn sync_on_network_change(&mut self) {
        let online = gio::NetworkMonitor::default().is_network_available();
        if self.sync.connectivity.update(online) {
            self.request_sync(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_network_coming_back_syncs() {
        let mut net = Connectivity::new(true);
        assert!(!net.update(true), "still online");
        assert!(!net.update(false), "lost");
        assert!(!net.update(false), "still offline");
        assert!(net.update(true), "back");
        assert!(!net.update(true), "once");
        let mut offline_at_launch = Connectivity::new(false);
        assert!(offline_at_launch.update(true));
    }

    #[test]
    fn syncs_every_fifteen_minutes() {
        assert_eq!(EVERY, Duration::from_secs(900));
    }
}
