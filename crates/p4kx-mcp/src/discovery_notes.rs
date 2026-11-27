// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeSet;

use p4kx_channels::DiscoveryIssue;
use p4kx_settings::SettingsError;

const NOTICE_BYTES: usize = 16 << 10;

thread_local! {
    static PENDING: RefCell<DiscoveryNotes> = RefCell::new(DiscoveryNotes::default());
}

/// Technical discovery notices are bounded independently of structured payloads.
#[derive(Default)]
struct DiscoveryNotes {
    messages: BTreeSet<String>,
    bytes: usize,
    omitted: usize,
}

impl DiscoveryNotes {
    fn note(&mut self, message: String) {
        if self.messages.contains(&message) {
            return;
        }
        if message.len() > NOTICE_BYTES.saturating_sub(self.bytes) {
            self.omitted += 1;
        } else {
            self.bytes += message.len();
            self.messages.insert(message);
        }
    }

    fn take(&mut self) -> Vec<String> {
        let previous = std::mem::take(self);
        let mut messages: Vec<String> = previous.messages.into_iter().collect();
        if previous.omitted != 0 {
            messages.push(format!(
                "Launcher discovery notices omitted: {} (notice limit: {} bytes)",
                previous.omitted, NOTICE_BYTES,
            ));
        }
        messages
    }
}

pub(crate) fn clear() {
    PENDING.with_borrow_mut(|pending| *pending = DiscoveryNotes::default());
}

pub(crate) fn remember(issues: &[DiscoveryIssue]) {
    PENDING.with_borrow_mut(|pending| {
        for issue in issues {
            pending.note(format!("Launcher discovery incomplete: {issue}"));
        }
    });
}

pub(crate) fn remember_settings(error: &SettingsError) {
    PENDING
        .with_borrow_mut(|pending| pending.note(format!("Launcher discovery incomplete: {error}")));
}

pub(crate) fn take() -> Vec<String> {
    PENDING.with_borrow_mut(DiscoveryNotes::take)
}

#[cfg(test)]
#[path = "discovery_notes_tests.rs"]
mod tests;
