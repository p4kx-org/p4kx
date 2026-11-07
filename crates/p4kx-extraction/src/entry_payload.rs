// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_archive::{Archive, Entry};

use crate::dataforge_pick::DataForgePick;
use crate::extract::{self, Rules};
use crate::source::SourceEntry;
use crate::write::EntryTarget;
use crate::{ExtractOptions, ExtractionReport};

/// Writes one Entry's decoded bytes unchanged as a portable `file_name` below `target`.
///
/// The Entry must belong to `archive`; its name is report metadata, conversion is disabled,
/// and other extraction options retain their usual semantics.
#[must_use]
pub fn extract_entry_payload(
    archive: &Archive,
    entry: &Entry,
    target: &Path,
    file_name: &str,
    options: &ExtractOptions<'_>,
) -> ExtractionReport {
    let options = ExtractOptions {
        convert: false,
        ..*options
    };
    let entries = [SourceEntry::of_archive(archive, entry)];
    let rules = Rules {
        entry_target: EntryTarget::Leaf(file_name),
        copy_other: true,
        pick: &|_, _| DataForgePick::default(),
    };
    extract::extract_files(archive, &entries, target, &options, &rules)
}
