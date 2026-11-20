// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_row::CatalogRow;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};
use crate::{CatalogError, ExportAccountingError, Kind};

type LocationLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

const SOURCE_TYPE: &str = "MissionLocality";

/// A physical mission locality and its original location slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MissionLocalityDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    available_locations: SourcedValue<Vec<LocationLink<'source>>>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> MissionLocalityDefinition<'source> {
    /// The physical Record header and its original field evidence.
    #[must_use]
    pub const fn header(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// The original location slots in stored order.
    #[must_use]
    pub const fn available_locations(&self) -> &SourcedValue<Vec<LocationLink<'source>>> {
        &self.available_locations
    }

    /// Other original fields retained under the same Record.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }
}

pub(crate) type PreparedMissionLocality<'source> =
    PreparedDefinition<'source, MissionLocalityDefinition<'source>>;

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedMissionLocality<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read_original(tree, input, admission, cancel, read_locality)
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut localities: Vec<PreparedMissionLocality<'source>>,
) -> CatalogTable<MissionLocalityDefinition<'source>> {
    PreparedDefinition::order(&mut localities);
    PreparedDefinition::table(forge, SOURCE_TYPE, localities)
}

pub(crate) fn catalog_rows<'rows>(
    rows: &'rows [PreparedMissionLocality<'_>],
) -> impl Iterator<Item = CatalogRow> + 'rows {
    PreparedDefinition::catalog_rows(rows, Kind::MissionLocalities)
}

pub(crate) fn sources<'rows, 'source>(
    rows: &'rows [PreparedMissionLocality<'source>],
) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
    PreparedDefinition::sources(Kind::MissionLocalities, rows)
}

fn read_locality<'source>(
    source: CanonicalRecordHeader<'source>,
    fields: Observed<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<MissionLocalityDefinition<'source>, CatalogError> {
    let available_locations = fields.record_link_array("availableLocations", notes)?;
    let authored_fields =
        fields.authored_members_except([("availableLocations", available_locations.state())])?;
    Ok(MissionLocalityDefinition {
        source,
        available_locations,
        authored_fields,
    })
}
