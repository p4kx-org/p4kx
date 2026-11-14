// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{
    DataForge, DecimalU64, PhysicalSourceRef, RecordTree, SourcedValue, Struct, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldCapture;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceRecordReference, SourceValueRef};
use crate::{CatalogError, ExportAccountingError};

const LOCAL_PARAMS: &str = "localGeneratorParams";
const AVAILABILITY: &str = "defaultAvailability";
const BOARD_AVAILABILITY: &str = "missionBoardAvailability";
const LOCAL_PARAMS_TYPE: &str = "LocalContractGeneratorParams";
const AVAILABILITY_TYPE: &str = "ContractAvailability";

type References<'source> = SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>;

/// The physical Handler occurrence in its original Generator arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LocalContractGeneratorSource {
    input_ref: PhysicalSourceRef,
    record_index: u32,
    handler: SourceValueRef,
}

/// Authored local generation and board settings at one Handler occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocalContractGeneratorConfiguration<'source> {
    source: LocalContractGeneratorSource,
    local_generator_params: SourcedValue<LocalContractGeneratorParams<'source>>,
    default_availability: SourcedValue<ContractBoardAvailability<'source>>,
}

impl<'source> LocalContractGeneratorConfiguration<'source> {
    fn read(
        source: LocalContractGeneratorSource,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source,
            local_generator_params: fields.strong_pointer_to(
                LOCAL_PARAMS,
                LOCAL_PARAMS_TYPE,
                &[LOCAL_PARAMS_TYPE],
                notes,
                LocalContractGeneratorParams::read,
            )?,
            default_availability: fields.declared_class_to(
                AVAILABILITY,
                AVAILABILITY_TYPE,
                notes,
                |fields, notes| {
                    Ok(ContractBoardAvailability {
                        mission_board_availability: fields.borrowed_enum_to(
                            BOARD_AVAILABILITY,
                            "MissionBoardAvailability",
                            notes,
                        )?,
                    })
                },
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ContractBoardAvailability<'source> {
    mission_board_availability: SourcedValue<&'source str>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct LocalContractGeneratorParams<'source> {
    scope: SourcedValue<&'source str>,
    parent_location_general_tags: SourcedValue<ContractLocationTags<'source>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    faction_identities: References<'source>,
}

impl<'source> LocalContractGeneratorParams<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            scope: fields.borrowed_enum_to("scope", "ContractScope", notes)?,
            parent_location_general_tags: fields.declared_class_to(
                "parentLocationGeneralTags",
                "TagsDNF",
                notes,
                |fields, notes| {
                    Ok(ContractLocationTags {
                        terms: fields.class_array_to(
                            "terms",
                            "TagsDNFTerm",
                            notes,
                            ContractLocationTagTerm::read,
                        )?,
                    })
                },
            )?,
            faction_identities: fields.record_link_array_to(
                "factionIdentities",
                "FactionIdentity",
                notes,
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct ContractLocationTags<'source> {
    terms: SourcedValue<Vec<SourcedValue<ContractLocationTagTerm<'source>>>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ContractLocationTagTerm<'source> {
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    positive_tags: References<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    negative_tags: References<'source>,
}

impl<'source> ContractLocationTagTerm<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            positive_tags: fields.record_link_array_to("positiveTags", "Tag", notes)?,
            negative_tags: fields.record_link_array_to("negativeTags", "Tag", notes)?,
        })
    }
}

pub(crate) fn read<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    capture: &dyn FieldCapture<'source>,
    notes: &mut Vec<FieldError>,
) -> Result<Vec<LocalContractGeneratorConfiguration<'source>>, CatalogError> {
    fields.check_tree(tree)?;
    let Some((_, slots)) = fields.field(None, "generators")? else {
        return Ok(Vec::new());
    };
    let Value::Array(slots) = slots.value() else {
        return Ok(Vec::new());
    };
    let numbers = RefCell::new(Vec::new());
    let root = Observed::definition(fields, input, capture, &numbers);
    let mut rows = Vec::new();
    for slot in slots {
        fields.check_cancel()?;
        let Value::Struct(handler) = slot else {
            continue;
        };
        let owner = fields
            .owner_of(handler)?
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if fields.field(Some(owner), LOCAL_PARAMS)?.is_none()
            && !board_is_declared(tree, fields, owner)?
        {
            continue;
        }
        let source = LocalContractGeneratorSource {
            input_ref: fields.input_ref().clone(),
            record_index: fields.record_index(),
            handler: SourceValueRef {
                value_index: DecimalU64::new(owner.ordinal()),
            },
        };
        rows.push(LocalContractGeneratorConfiguration::read(
            source,
            root.at_owner(owner),
            notes,
        )?);
    }
    Ok(rows)
}

fn board_is_declared<'scope, 'read, 'source>(
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    owner: &OriginalValueFact<'scope, 'read, 'source>,
) -> Result<bool, CatalogError> {
    let Some((_, fact)) = fields.field(Some(owner), AVAILABILITY)? else {
        return Ok(false);
    };
    if let Value::Struct(body) = fact.value() {
        return Ok(body.field(BOARD_AVAILABILITY).is_some());
    }
    let target = u32::from(fact.field().storage().target());
    Ok(tree.struct_at(target).is_some_and(|structure| {
        matches!(structure.name(), Ok(AVAILABILITY_TYPE))
            && matches!(
                board_field_support(&structure),
                DatasetAvailability::Available
            )
    }))
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<LocalContractGeneratorConfiguration<'source>>,
) -> CatalogTable<LocalContractGeneratorConfiguration<'source>> {
    let availability = if rows.is_empty() {
        source_availability(forge)
    } else {
        DatasetAvailability::Available
    };
    CatalogTable { availability, rows }
}

fn source_availability(forge: &DataForge<'_>) -> DatasetAvailability {
    let mut availability = DatasetAvailability::Absent;
    for structure in forge.structs() {
        let declaration = match structure.name() {
            Ok(LOCAL_PARAMS_TYPE) => DatasetAvailability::Available,
            Ok(AVAILABILITY_TYPE) => board_field_support(&structure),
            Ok(_) => DatasetAvailability::Absent,
            Err(_) => DatasetAvailability::Unavailable,
        };
        match declaration {
            DatasetAvailability::Available => return DatasetAvailability::Available,
            DatasetAvailability::Unavailable => availability = DatasetAvailability::Unavailable,
            DatasetAvailability::Absent => {}
        }
    }
    availability
}

fn board_field_support(structure: &Struct<'_>) -> DatasetAvailability {
    let mut availability = DatasetAvailability::Absent;
    for property in structure.properties() {
        match property.name() {
            Ok(BOARD_AVAILABILITY) => return DatasetAvailability::Available,
            Ok(_) => {}
            Err(_) => availability = DatasetAvailability::Unavailable,
        }
    }
    availability
}
