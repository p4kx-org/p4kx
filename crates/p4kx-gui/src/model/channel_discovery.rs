// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use crate::ChannelList;
use crate::channel_refresh::RefreshFailure;

use super::Model;

impl Model {
    /// Replaces the Channels the picker offers and invalidates older refresh results.
    pub fn set_channels(&mut self, channels: ChannelList) {
        self.channel_refresh.invalidate();
        self.channels = channels;
        self.protect_manual_entries();
    }

    /// Registers a manual entry immediately and queues its background discovery refresh.
    pub fn add_manual(&mut self, path: PathBuf) -> bool {
        self.add_manual_with_wake(path, || {})
    }

    /// Registers a manual entry and calls `wake` when its background refresh can be polled.
    pub fn add_manual_with_wake(
        &mut self,
        path: PathBuf,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> bool {
        let added = self.channels.add(path);
        self.protect_manual_entries();
        if added {
            self.channel_refresh.request(self.channels.search(), wake);
        }
        added
    }

    fn protect_manual_entries(&self) {
        let manual = self.channels.manual();
        p4kx_extraction::protect_manual(&manual.folders, &manual.archives);
    }

    pub(super) fn poll_channel_refresh(&mut self) {
        if let Some(measured) = self.channel_refresh.poll() {
            self.channels.adopt(measured);
        }
    }

    pub(crate) const fn channel_refresh_failure(&self) -> Option<&RefreshFailure> {
        self.channel_refresh.failure()
    }
}

#[cfg(test)]
#[path = "channel_discovery_tests.rs"]
mod tests;
