// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Selects notification recipients from observed thread and process identities.

use p4kx_msi_path::notification::FailureKind;

/// One thread and its process.
#[derive(Clone, Copy)]
pub(crate) struct Owner {
    pub(crate) thread: u32,
    pub(crate) process: u32,
}

/// Whether an observed window requires an external notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Recipient {
    OwnProcess,
    External,
}

/// Excludes only a proven helper-process window.
///
/// # Errors
/// Returns `CurrentThread` when own-process exclusion is unproven for the calling thread.
pub(crate) const fn classify(owner: Owner, helper: Owner) -> Result<Recipient, FailureKind> {
    if owner.thread != 0 && helper.process != 0 && owner.process == helper.process {
        return Ok(Recipient::OwnProcess);
    }
    if owner.thread == helper.thread {
        return Err(FailureKind::CurrentThread);
    }
    Ok(Recipient::External)
}

#[cfg(test)]
#[path = "notification_recipient_tests.rs"]
mod tests;
