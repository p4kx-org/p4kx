// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_dataforge::{DataForge, PositionedRecord, Record, RecordJson};

use super::write_observed;
use super::writer::Observation;
use crate::{
    DataForgeEntry, ExtractionError, ObservedTagTree, RecordExportOptions, RecordExportReport,
    RecordExportSources, RecordFilePlan, RecordJsonReceipt, RecordLocalization,
    SourceEvidenceError,
};

/// Writes each selected unannotated Record JSON file and its origin companion.
///
/// # Errors
/// Returns an error if the options contain annotations without observed sources.
pub fn export_record_json_files_with_origins(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    write_observed(
        forge,
        records,
        target,
        options,
        Observation::new(input, &RecordJsonReceipt::render_file),
        None,
    )
}

/// Writes Record JSON files with their observed annotations and field-origin companions.
///
/// # Errors
/// Returns an error if the options supply independent annotations or the tag tree is foreign.
pub fn export_annotated_record_json_files(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    localization: Option<&RecordLocalization<'_>>,
    tags: Option<&ObservedTagTree>,
) -> Result<RecordExportReport, ExtractionError> {
    export_observed(
        RecordExportSources::new(input, localization, tags),
        forge,
        records,
        target,
        options,
        None,
    )
}

/// Writes selected Record JSON files from a cached plan and retains definition indices.
///
/// # Errors
/// Returns an error for foreign tags or independently supplied annotations.
pub fn export_planned_record_json_files_with_origins(
    sources: RecordExportSources<'_, '_>,
    forge: &DataForge<'_>,
    plan: &RecordFilePlan<RecordJson>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    export_observed(sources, forge, records, target, options, Some(plan))
}

fn export_observed(
    sources: RecordExportSources<'_, '_>,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    plan: Option<&RecordFilePlan<RecordJson>>,
) -> Result<RecordExportReport, ExtractionError> {
    let RecordExportSources {
        input,
        localization,
        tags,
    } = sources;
    if tags.is_some_and(|tags| !tags.is_from(forge)) {
        return Err(crate::write::write_error(
            target.to_path_buf(),
            io::Error::new(
                io::ErrorKind::InvalidInput,
                SourceEvidenceError::ForeignEvidence,
            ),
        ));
    }
    let render = |writer: &mut RecordJson,
                  input: &DataForgeEntry,
                  forge: &DataForge<'_>,
                  record: &PositionedRecord<'_>| {
        RecordJsonReceipt::render_annotated_file(writer, input, forge, record, localization, tags)
    };
    write_observed(
        forge,
        records,
        target,
        options,
        Observation::new(input, &render),
        plan,
    )
}
