// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::entry_identity::PhysicalEntryId;
use crate::key_demand::KeyDemandSnapshot;

#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyFailure {
    Missing,
    Wrong,
}

#[derive(Debug, Default)]
struct Reasons {
    missing: bool,
    wrong: bool,
}

impl Reasons {
    const fn insert(&mut self, cause: KeyFailure) -> bool {
        let previous = match cause {
            KeyFailure::Missing => std::mem::replace(&mut self.missing, true),
            KeyFailure::Wrong => std::mem::replace(&mut self.wrong, true),
        };
        !previous
    }
}

#[derive(Debug, Default)]
pub(crate) struct KeyDemandCounts {
    entries: HashMap<PhysicalEntryId, Reasons>,
    missing: usize,
    wrong: usize,
    first_missing: Option<String>,
    first_wrong: Option<String>,
}

impl KeyDemandCounts {
    pub(crate) fn add(&mut self, entry: &PhysicalEntryId, path: &str, cause: KeyFailure) {
        if !self.entries.entry(entry.clone()).or_default().insert(cause) {
            return;
        }
        let (count, first) = match cause {
            KeyFailure::Missing => (&mut self.missing, &mut self.first_missing),
            KeyFailure::Wrong => (&mut self.wrong, &mut self.first_wrong),
        };
        *count += 1;
        if first.is_none() {
            *first = Some(path.to_owned());
        }
    }

    pub(crate) fn snapshot(&self) -> KeyDemandSnapshot {
        KeyDemandSnapshot {
            missing: self.missing,
            wrong: self.wrong,
            first_missing: self.first_missing.clone(),
            first_wrong: self.first_wrong.clone(),
            distinct_unread: self.entries.len(),
        }
    }
}
