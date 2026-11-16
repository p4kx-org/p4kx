// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_archive::{Archive, Entry};

use crate::dataforge_pick::DataForgePick;
use crate::extract::{self, Rules};
use crate::source::{EntrySource, SourceEntry};
use crate::verified_entry_payload::{VerifiedEntryPayload, VerifiedPayloadSource};
use crate::write::EntryTarget;
use crate::{EntryFailure, ExtractOptions, ExtractionError, ExtractionReport};

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
    extract_payload(
        archive,
        SourceEntry::of_archive(archive, entry),
        target,
        file_name,
        options,
    )
}

/// Writes one verified Entry unchanged and seals the decoded bytes of the completed write.
///
/// The proof is absent for a kept file, an interrupted or failed write, or an incomplete read.
/// The original Entry must belong to this Archive.
#[must_use]
pub fn extract_verified_entry_payload(
    archive: &Archive,
    entry: &Entry,
    target: &Path,
    file_name: &str,
    options: &ExtractOptions<'_>,
) -> (ExtractionReport, Option<VerifiedEntryPayload>) {
    let Some(source) = VerifiedPayloadSource::new(archive, entry) else {
        let failure = EntryFailure {
            path: entry.path().to_owned(),
            error: ExtractionError::Read(io::Error::other(
                "the physical Entry belongs to another Archive",
            )),
        };
        return (
            ExtractionReport {
                failures: vec![failure],
                ..ExtractionReport::default()
            },
            None,
        );
    };
    let options = ExtractOptions {
        verify: true,
        ..*options
    };
    let report = extract_payload(&source, source.source_entry(), target, file_name, &options);
    let proof = if report.is_complete() && report.extracted() == 1 && report.skipped().is_empty() {
        source.into_proof()
    } else {
        None
    };
    (report, proof)
}

fn extract_payload(
    source: &dyn EntrySource,
    entry: SourceEntry<'_>,
    target: &Path,
    file_name: &str,
    options: &ExtractOptions<'_>,
) -> ExtractionReport {
    let options = ExtractOptions {
        convert: false,
        ..*options
    };
    let entries = [entry];
    let rules = Rules {
        entry_target: EntryTarget::Leaf(file_name),
        copy_other: true,
        pick: &|_, _| DataForgePick::default(),
    };
    extract::extract_files(source, &entries, target, &options, &rules)
}
