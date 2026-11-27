// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_dataforge::{DataForge, PositionedRecord, Record, RecordXml};

use super::write_observed;
use super::writer::Observation;
use crate::record_format::xml_receipt::{self, XmlAnnotationSources};
use crate::write::write_error;
use crate::{
    DataForgeEntry, ExtractionError, ObservedTagTree, RecordExportOptions, RecordExportReport,
    RecordExportSources, RecordFilePlan, RecordLocalization,
};

/// Writes each selected unannotated Record XML file with its exact field companion.
///
/// # Errors
/// Returns an error if the options require unobserved annotation sources.
pub fn export_record_xml_files_with_origins(
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
        Observation::new(input, &xml_receipt::render),
        None,
    )
}

/// Writes each selected Record XML file and its exact localization companion.
///
/// # Errors
/// Returns an error if the options contain an unobserved annotation.
pub fn export_localized_record_xml_files(
    input: &DataForgeEntry,
    forge: &DataForge<'_>,
    records: &[Record<'_>],
    target: &Path,
    options: &RecordExportOptions<'_>,
    localization: &RecordLocalization<'_>,
) -> Result<RecordExportReport, ExtractionError> {
    export_annotated_record_xml_files(
        input,
        forge,
        records,
        target,
        options,
        Some(localization),
        None,
    )
}

/// Writes selected Record XML files with their observed annotations and field-origin companions.
///
/// # Errors
/// Returns an error for a foreign tag tree or an independently supplied annotation.
pub fn export_annotated_record_xml_files(
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

/// Writes selected Record XML files from a cached plan and retains definition indices.
///
/// # Errors
/// Returns an error for foreign tags or independently supplied annotations.
pub fn export_planned_record_xml_files_with_origins(
    sources: RecordExportSources<'_, '_>,
    forge: &DataForge<'_>,
    plan: &RecordFilePlan<RecordXml>,
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
    plan: Option<&RecordFilePlan<RecordXml>>,
) -> Result<RecordExportReport, ExtractionError> {
    let RecordExportSources {
        input,
        localization,
        tags,
    } = sources;
    let sources = XmlAnnotationSources { localization, tags };
    sources
        .validate(forge)
        .map_err(|source| write_error(target.to_path_buf(), source))?;
    let render = |writer: &mut RecordXml,
                  input: &DataForgeEntry,
                  forge: &DataForge<'_>,
                  record: &PositionedRecord<'_>| {
        xml_receipt::render_annotated(writer, input, forge, record, sources)
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
