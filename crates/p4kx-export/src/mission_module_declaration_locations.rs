// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

const STRATEGY: &str = "ModuleLocationEntitiesAcquiringStrategy_Base";
type EntityLinks<'source> = SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ModuleLocationEntities<'source> {
    source_type: SourcedValue<&'source str>,
    debug_name: SourcedValue<&'source str>,
    required_for_init: SourcedValue<bool>,
    optional: SourcedValue<bool>,
    acquiring_strategy: SourcedValue<AcquiringStrategy<'source>>,
    #[serde(flatten)]
    data: LocationEntityData<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum LocationEntityData<'source> {
    Static {
        #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
        entity_declarations: EntityLinks<'source>,
    },
    Authored,
}

impl<'source> ModuleLocationEntities<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let debug_name = fields.borrowed_string("debugName", notes)?;
        let required_for_init = fields.boolean("requiredForInit", notes)?;
        let optional = fields.boolean("optional", notes)?;
        let acquiring_strategy = fields.strong_pointer_family_to(
            "acquiringStrategy",
            STRATEGY,
            notes,
            AcquiringStrategy::read,
        )?;
        let (data, projected) = if fields.source_type()? == "ModuleLocationEntities_Static" {
            let links = fields.record_link_array_to(
                "entityDeclarations",
                "LocationEntityDeclaration",
                notes,
            )?;
            let projected = Some(("entityDeclarations", links.state()));
            (
                LocationEntityData::Static {
                    entity_declarations: links,
                },
                projected,
            )
        } else {
            fields.note_retained_variant(notes)?;
            (LocationEntityData::Authored, None)
        };
        let shared = [
            ("debugName", debug_name.state()),
            ("requiredForInit", required_for_init.state()),
            ("optional", optional.state()),
            ("acquiringStrategy", acquiring_strategy.state()),
        ];
        Ok(Self {
            source_type: fields.source_type_value()?,
            debug_name,
            required_for_init,
            optional,
            acquiring_strategy,
            data,
            authored_fields: fields.authored_members_except(shared.into_iter().chain(projected))?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct AcquiringStrategy<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    data: StrategyData,
    authored_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum StrategyData {
    ExactCount {
        count: SourcedValue<i32>,
    },
    AllSuitable {
        min_count: SourcedValue<i32>,
        max_count: SourcedValue<i32>,
    },
    Authored,
}

impl<'source> AcquiringStrategy<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let (data, projected) = match fields.source_type()? {
            "ModuleLocationEntitiesAcquiringStrategy_ExactCount" => {
                let count = fields.int32("count", notes)?;
                let projected = [Some(("count", count.state())), None];
                (StrategyData::ExactCount { count }, projected)
            }
            "ModuleLocationEntitiesAcquiringStrategy_AllSuitable" => {
                let min_count = fields.int32("minCount", notes)?;
                let max_count = fields.int32("maxCount", notes)?;
                let projected = [
                    Some(("minCount", min_count.state())),
                    Some(("maxCount", max_count.state())),
                ];
                (
                    StrategyData::AllSuitable {
                        min_count,
                        max_count,
                    },
                    projected,
                )
            }
            _ => {
                fields.note_retained_variant(notes)?;
                (StrategyData::Authored, [None; 2])
            }
        };
        Ok(Self {
            source_type: fields.source_type_value()?,
            data,
            authored_fields: fields.authored_members_except(projected.into_iter().flatten())?,
        })
    }
}
