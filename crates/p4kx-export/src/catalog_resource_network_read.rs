// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::ops::Range;

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree, TreeBuilder, Value};

use super::authored_fields::Observed;
use super::catalog_resource_network_configuration::{
    GAME_MODE, NETWORK, PARAMS, ResourceNetworkConfiguration,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_field_walk;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::export_source_failure;
use crate::record_observation::RecordObservation;
use crate::source_projection_notes;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure};

const ROLE: ExportSourceRole = ExportSourceRole::Dependency;

pub(crate) struct ResourceNetworkRead<'source> {
    pub(crate) table: CatalogTable<ResourceNetworkConfiguration<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
    pub(crate) ledger: ExportLedger,
}

/// Reads the original game-mode owners that match the catalog's Record filter.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<ResourceNetworkRead<'source>, CatalogError> {
    input.check_forge(forge)?;
    let mut trees = TreeBuilder::new();
    let mut rows = Vec::new();
    let mut numbers = Vec::new();
    let mut failures = Vec::new();
    let mut ledger = ExportLedger::default();
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if record.type_name().ok() != Some(GAME_MODE) {
            continue;
        }
        if let Some(filter) = filter {
            match record.record_path() {
                Ok(path) if !filter.matches(path) => continue,
                Ok(_) => {}
                Err(error) => {
                    let admission = RecordObservation::capture(input, &record)?;
                    ledger.observe(
                        admission
                            .source(
                                None,
                                ExportSourceRole::SelectionCandidate,
                                SourceDisposition::ReadFailed,
                            )?
                            .diagnosed(export_source_failure::diagnosis(&error)),
                    );
                    failures.push(CatalogReadFailure::of(&record, &error));
                    continue;
                }
            }
        }
        let admission = RecordObservation::capture(input, &record)?;
        match trees.build(&record) {
            Ok(tree) => {
                let projected = project(&tree, input, &admission, cancel, &mut ledger)?;
                rows.push(projected.row);
                numbers.extend(projected.numbers);
            }
            Err(error) => {
                ledger.observe(
                    admission
                        .source(None, ROLE, SourceDisposition::ReadFailed)?
                        .diagnosed(export_source_failure::diagnosis(&error)),
                );
                failures.push(CatalogReadFailure::of(&record, &error));
            }
        }
    }
    Ok(ResourceNetworkRead {
        table: CatalogTable::new(forge, &[GAME_MODE], rows),
        numbers,
        failures,
        ledger,
    })
}

struct NetworkProjection<'source> {
    row: ResourceNetworkConfiguration<'source>,
    numbers: Vec<CapturedNumber>,
}

fn project<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    ledger: &mut ExportLedger,
) -> Result<NetworkProjection<'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let receiver = Observed::configuration(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let row = ResourceNetworkConfiguration::read(source, receiver, &mut notes)?;
    let span = rule_span(&originals)?;
    ledger.merge(export_field_walk::inspect_configuration(
        input,
        tree,
        admission,
        span.as_slice(),
        ConfigurationInspection::ResourceNetwork,
        cancel,
    )?);
    source_projection_notes::record_dependency(admission, tree, notes, ledger)?;
    ledger.observe(
        admission
            .source(None, ROLE, SourceDisposition::Projected)?
            .for_configuration(ConfigurationInspection::ResourceNetwork),
    );
    Ok(NetworkProjection {
        row,
        numbers: numbers.into_inner(),
    })
}

fn rule_span(
    originals: &OriginalFlowFields<'_, '_, '_>,
) -> Result<Option<Range<usize>>, CatalogError> {
    let Some((_, params)) = originals.field(None, PARAMS)? else {
        return Ok(None);
    };
    if !matches!(params.value(), Value::Struct(_)) {
        return Ok(None);
    }
    originals
        .field(Some(params), NETWORK)?
        .map(|(_, rules)| originals.span(rules))
        .transpose()
}
