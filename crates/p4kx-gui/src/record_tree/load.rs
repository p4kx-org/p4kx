// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_extraction::DataForgeError;

use super::DataForgeFailure;
use crate::EntryIndex;
use crate::forge_cache::ReadFailure;
use crate::model::describe;
use crate::record_index::RecordIndex;

pub(super) enum Outcome {
    Loaded(Arc<RecordIndex>),
    Failed(DataForgeFailure),
    ArchiveChanged,
}

pub(super) fn load(index: &EntryIndex, cancelled: &AtomicBool) -> Outcome {
    if index.watch().check().is_err() {
        return Outcome::ArchiveChanged;
    }
    let source = match index.forge().read(index, || {}) {
        Ok(source) => source,
        Err(ReadFailure::ArchiveChanged(_)) => return Outcome::ArchiveChanged,
        Err(ReadFailure::Unreadable(reason)) => {
            return Outcome::Failed(DataForgeFailure::Unreadable(reason));
        }
    };
    if cancelled.load(Ordering::Relaxed) {
        return Outcome::Failed(DataForgeFailure::Stopped);
    }
    let built = source.entry.open().map(|forge| {
        let actual = source.entry.origin_of(&forge);
        RecordIndex::build(&forge, actual)
    });
    if index.watch().check().is_err() {
        return Outcome::ArchiveChanged;
    }
    match built {
        Ok(records) => Outcome::Loaded(Arc::new(records)),
        Err(DataForgeError::UnsupportedVersion { version }) => {
            Outcome::Failed(DataForgeFailure::Unsupported { version })
        }
        Err(error) => Outcome::Failed(DataForgeFailure::Unreadable(describe(&error))),
    }
}
