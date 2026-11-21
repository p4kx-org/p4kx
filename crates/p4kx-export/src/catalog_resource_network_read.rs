// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::ops::Range;

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree, TreeBuilder, Value};

use super::authored_fields::Observed;
use super::catalog_radar_options::{RADAR, RadarOptionsConfiguration};
use super::catalog_resource_network_configuration::{
    self, GAME_MODE, GameModeConfigurations, NETWORK, PARAMS, ResourceNetworkConfiguration,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::CatalogTable;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_field_walk;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::export_source_failure;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::source_projection_notes;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure};

#[path = "resource_network_global.rs"]
pub(crate) mod resource_network_global;

const ROLE: ExportSourceRole = ExportSourceRole::Dependency;
const NETWORK_RULE: (&str, ConfigurationInspection) =
    (NETWORK, ConfigurationInspection::ResourceNetwork);
const RADAR_RULE: (&str, ConfigurationInspection) = (RADAR, ConfigurationInspection::RadarOptions);
pub(crate) const GAME_MODE_TYPES: &[&str] = &[GAME_MODE];

pub(super) struct GameModeRead<'source> {
    pub(super) resource_network_parameters: CatalogTable<ResourceNetworkConfiguration<'source>>,
    pub(super) radar_options: CatalogTable<RadarOptionsConfiguration<'source>>,
    pub(crate) numbers: Vec<CapturedNumber>,
    pub(crate) failures: Vec<CatalogReadFailure<'source>>,
    pub(crate) ledger: ExportLedger,
}

/// Reads the original game-mode owners that match the catalog's Record filter.
pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<GameModeRead<'source>, CatalogError> {
    input.check_forge(forge)?;
    let mut trees = TreeBuilder::new();
    let mut network = Vec::new();
    let mut radar = Vec::new();
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
                let projected = project_combined(&tree, input, &admission, cancel)?;
                network.push(projected.rows.network);
                radar.push(projected.rows.radar);
                numbers.extend(projected.numbers);
                ledger.merge(projected.ledger);
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
    Ok(GameModeRead {
        resource_network_parameters: CatalogTable::new(forge, GAME_MODE_TYPES, network),
        radar_options: CatalogTable::new(forge, GAME_MODE_TYPES, radar),
        numbers,
        failures,
        ledger,
    })
}

fn project_combined<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ModeProjection<GameModeConfigurations<'source>>, CatalogError> {
    project(
        tree,
        input,
        admission,
        cancel,
        &[NETWORK_RULE, RADAR_RULE],
        GameModeConfigurations::read,
    )
}

pub(crate) fn read_radar_selected<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<RadarOptionsConfiguration<'source>>, CatalogError> {
    Ok(project(
        tree,
        input,
        admission,
        cancel,
        &[RADAR_RULE],
        catalog_resource_network_configuration::read_radar,
    )?
    .into_scoped_read())
}

pub(crate) fn read_network_selected<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<ResourceNetworkConfiguration<'source>>, CatalogError> {
    Ok(project(
        tree,
        input,
        admission,
        cancel,
        &[NETWORK_RULE],
        catalog_resource_network_configuration::read_network,
    )?
    .into_scoped_read())
}

struct ModeProjection<Row> {
    rows: Row,
    numbers: Vec<CapturedNumber>,
    ledger: ExportLedger,
}

impl<Row> ModeProjection<Row> {
    fn into_scoped_read(self) -> ScopedSourceRead<Row> {
        let complete = self.ledger.field_projection_complete(0, 0);
        ScopedSourceRead::new(self.rows, Vec::new(), complete)
    }
}

fn project<'source, Row>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    rules: &[(&str, ConfigurationInspection)],
    read: impl FnOnce(
        CanonicalRecordHeader<'source>,
        Observed<'_, '_, '_, 'source>,
        &mut Vec<FieldError>,
    ) -> Result<Row, CatalogError>,
) -> Result<ModeProjection<Row>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let receiver = Observed::configuration(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let rows = read(source, receiver, &mut notes)?;
    let mut ledger = ExportLedger::default();
    for &(field, configuration) in rules {
        let span = rule_span(&originals, field)?;
        ledger.merge(export_field_walk::inspect_configuration(
            input,
            tree,
            admission,
            span.as_slice(),
            configuration,
            cancel,
        )?);
        ledger.observe(
            admission
                .source(None, ROLE, SourceDisposition::Projected)?
                .for_configuration(configuration),
        );
    }
    source_projection_notes::record_dependency(admission, tree, notes, &mut ledger)?;
    Ok(ModeProjection {
        rows,
        numbers: numbers.into_inner(),
        ledger,
    })
}

fn rule_span(
    originals: &OriginalFlowFields<'_, '_, '_>,
    field: &str,
) -> Result<Option<Range<usize>>, CatalogError> {
    let Some((_, params)) = originals.field(None, PARAMS)? else {
        return Ok(None);
    };
    if !matches!(params.value(), Value::Struct(_)) {
        return Ok(None);
    }
    originals
        .field(Some(params), field)?
        .map(|(_, rules)| originals.span(rules))
        .transpose()
}
