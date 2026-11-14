// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeSet;

use p4kx_extraction::{
    CancelToken, DataForge, DataForgeError, Filter, Record, RecordTree, TreeBuilder,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_jump_drive::{FLIGHT_PARAMS, JumpDrive, TUNNEL_FORCES};
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
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceArenaOutput;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, CatalogReadFailure};

const ROLE: ExportSourceRole = ExportSourceRole::Dependency;
const INSPECTION: ConfigurationInspection = ConfigurationInspection::JumpDriveDefinition;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct JumpDriveDefinition<'source> {
    source: CanonicalRecordHeader<'source>,
    fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct JumpDriveDefinitions<'source> {
    pub(crate) jump_drive_flight_tuning: CatalogTable<JumpDriveDefinition<'source>>,
    pub(crate) jump_tunnel_forces: CatalogTable<JumpDriveDefinition<'source>>,
}

#[derive(Default)]
pub(super) struct JumpDriveDefinitionRead<'source> {
    flight: Vec<JumpDriveDefinition<'source>>,
    tunnel: Vec<JumpDriveDefinition<'source>>,
    pub(super) numbers: Vec<CapturedNumber>,
    pub(super) failures: Vec<CatalogReadFailure<'source>>,
    pub(super) ledger: ExportLedger,
}

impl<'source> JumpDriveDefinitionRead<'source> {
    pub(super) fn take_tables(&mut self, forge: &DataForge<'_>) -> JumpDriveDefinitions<'source> {
        JumpDriveDefinitions {
            jump_drive_flight_tuning: CatalogTable::new(
                forge,
                &[FLIGHT_PARAMS],
                std::mem::take(&mut self.flight),
            ),
            jump_tunnel_forces: CatalogTable::new(
                forge,
                &[TUNNEL_FORCES],
                std::mem::take(&mut self.tunnel),
            ),
        }
    }

    fn failed(
        &mut self,
        admission: &RecordObservation<'_, 'source>,
        role: ExportSourceRole,
        error: &DataForgeError,
    ) -> Result<(), CatalogError> {
        self.ledger.observe(
            admission
                .source(None, role, SourceDisposition::ReadFailed)?
                .for_configuration(INSPECTION)
                .diagnosed(export_source_failure::diagnosis(error)),
        );
        self.failures
            .push(CatalogReadFailure::of(admission.record(), error));
        Ok(())
    }
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    devices: &[JumpDrive<'source>],
) -> Result<JumpDriveDefinitionRead<'source>, CatalogError> {
    input.check_forge(forge)?;
    let linked: BTreeSet<_> = devices
        .iter()
        .flat_map(JumpDrive::linked_definitions)
        .collect();
    let mut result = JumpDriveDefinitionRead::default();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let Ok(name @ (FLIGHT_PARAMS | TUNNEL_FORCES)) = record.type_name() else {
            continue;
        };
        match selected(&record, filter, &linked) {
            Ok(false) => continue,
            Err(error) => {
                let admission = RecordObservation::capture(input, &record)?;
                result.failed(&admission, ExportSourceRole::SelectionCandidate, &error)?;
                continue;
            }
            Ok(true) => {}
        }
        let admission = RecordObservation::capture(input, &record)?;
        match trees.build(&record) {
            Ok(tree) => {
                let row = project(&tree, input, &admission, cancel, &mut result)?;
                if name == FLIGHT_PARAMS {
                    result.flight.push(row);
                } else {
                    result.tunnel.push(row);
                }
            }
            Err(error) => result.failed(&admission, ROLE, &error)?,
        }
    }
    Ok(result)
}

fn selected(
    record: &Record<'_>,
    filter: Option<&Filter>,
    linked: &BTreeSet<u32>,
) -> Result<bool, DataForgeError> {
    if let Some(filter) = filter
        && !linked.contains(&record.index())
    {
        return Ok(filter.matches(record.record_path()?));
    }
    Ok(true)
}

fn project<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    result: &mut JumpDriveDefinitionRead<'source>,
) -> Result<JumpDriveDefinition<'source>, CatalogError> {
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let observed = Observed::configuration(&originals, input, &capture, &numbers);
    let source = CanonicalRecordHeader::read(tree, input.origins())?;
    let fields = observed.authored_members()?;
    result.numbers.extend(numbers.into_inner());
    let values = 0..originals.facts().len();
    let spans = (!values.is_empty()).then_some(values);
    result
        .ledger
        .merge(export_field_walk::inspect_configuration(
            input,
            tree,
            admission,
            spans.as_slice(),
            INSPECTION,
            cancel,
        )?);
    result.ledger.observe(
        admission
            .source(None, ROLE, SourceDisposition::Projected)?
            .for_configuration(INSPECTION),
    );
    Ok(JumpDriveDefinition { source, fields })
}
