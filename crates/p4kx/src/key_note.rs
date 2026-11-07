// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The one note a run prints on stderr when encrypted Entries stayed unread for want of the Archive
//! key.

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use p4kx_extraction::{KeyDemand, KeyDemandSnapshot};

use crate::key::{REQUIRED_HINT, WRONG_HINT};

static DEMANDS: Mutex<Vec<KeyDemand>> = Mutex::new(Vec::new());
static SUPPRESSED: AtomicBool = AtomicBool::new(false);

/// Follows the key demand of an Archive the run opened.
pub(crate) fn watch(demand: KeyDemand) {
    DEMANDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(demand);
}

/// Drops the note because the run already failed with a key error and said so.
pub(crate) fn suppress() {
    SUPPRESSED.store(true, Ordering::Relaxed);
}

/// Prints the note on stderr when any Archive of the run left encrypted Entries unread.
pub(crate) fn print() {
    if SUPPRESSED.load(Ordering::Relaxed) {
        return;
    }
    let seen: Vec<KeyDemandSnapshot> = DEMANDS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(KeyDemand::snapshot)
        .collect();
    if let Some(note) = note(&seen) {
        let _ignored = writeln!(io::stderr().lock(), "{note}");
    }
}

/// Returns the note for `seen`, `None` when no encrypted Entry went unread.
fn note(seen: &[KeyDemandSnapshot]) -> Option<String> {
    let unread: usize = seen.iter().map(KeyDemandSnapshot::unread).sum();
    if unread == 0 {
        return None;
    }
    let wrong = seen.iter().any(|snapshot| snapshot.wrong > 0);
    let first = seen
        .iter()
        .find_map(|snapshot| {
            if wrong {
                snapshot.first_wrong.as_deref()
            } else {
                snapshot.first_missing.as_deref()
            }
        })
        .unwrap_or_default();
    let (cause, hint) = if wrong {
        ("the Archive key does not fit", WRONG_HINT)
    } else {
        ("no Archive key", REQUIRED_HINT)
    };
    let entries = if unread == 1 {
        "1 encrypted Entry was".to_owned()
    } else {
        format!("{unread} encrypted Entries were")
    };
    Some(format!(
        "note: {cause}: {entries} not read (the first is {first}), so the result is incomplete; \
         {hint}"
    ))
}
