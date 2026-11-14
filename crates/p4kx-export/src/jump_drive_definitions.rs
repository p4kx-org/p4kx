// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_jump_drive::{FLIGHT_PARAMS, JumpDrive, TUNNEL_FORCES};
use super::referenced_definition_read::{self, Selection};
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_table::CatalogTable;
use crate::export_field_inspection::ConfigurationInspection;
use crate::export_ledger::ExportLedger;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, CatalogReadFailure};

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

pub(super) struct JumpDriveDefinitionRead<'source> {
    pub(super) tables: JumpDriveDefinitions<'source>,
    pub(super) numbers: Vec<CapturedNumber>,
    pub(super) failures: Vec<CatalogReadFailure<'source>>,
    pub(super) ledger: ExportLedger,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    devices: &[JumpDrive<'source>],
) -> Result<JumpDriveDefinitionRead<'source>, CatalogError> {
    let linked = devices
        .iter()
        .flat_map(JumpDrive::linked_definitions)
        .collect();
    let read = referenced_definition_read::read(
        forge,
        input,
        &Selection {
            source_types: &[FLIGHT_PARAMS, TUNNEL_FORCES],
            linked: &linked,
            filter,
            inspection: ConfigurationInspection::JumpDriveDefinition,
        },
        cancel,
        project,
    )?;
    let (flight, tunnel) = read
        .rows
        .into_iter()
        .partition::<Vec<_>, _>(|(name, _)| *name == FLIGHT_PARAMS);
    let rows = |entries: Vec<(_, _)>| entries.into_iter().map(|(_, row)| row).collect();
    Ok(JumpDriveDefinitionRead {
        tables: JumpDriveDefinitions {
            jump_drive_flight_tuning: CatalogTable::new(forge, &[FLIGHT_PARAMS], rows(flight)),
            jump_tunnel_forces: CatalogTable::new(forge, &[TUNNEL_FORCES], rows(tunnel)),
        },
        numbers: read.numbers,
        failures: read.failures,
        ledger: read.ledger,
    })
}

fn project<'source>(
    _source_type: &'source str,
    source: CanonicalRecordHeader<'source>,
    observed: Observed<'_, '_, '_, 'source>,
    _notes: &mut Vec<FieldError>,
) -> Result<JumpDriveDefinition<'source>, CatalogError> {
    Ok(JumpDriveDefinition {
        source,
        fields: observed.authored_members()?,
    })
}
