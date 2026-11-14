// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::catalog_durability::resistance::{INLINE, ResistanceSet};
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const COMPONENT: &str = "SCItemSuitArmorParams";
pub(super) const MACRO: &str = "DamageResistanceMacro";

/// One original suit armor component with its resistance definition link.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SuitArmor<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    integrity_milestone_to_break: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    damage_resistance: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> SuitArmor<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let integrity_milestone_to_break = source.single("integrityMilestoneToBreak", notes)?;
        let damage_resistance = source.record_link_to("damageResistance", MACRO, notes)?;
        let original = UnitSource::read(
            source,
            [
                (
                    "integrityMilestoneToBreak",
                    integrity_milestone_to_break.state(),
                ),
                ("damageResistance", damage_resistance.state()),
            ],
        )?;
        Ok(Self {
            owner,
            integrity_milestone_to_break,
            damage_resistance,
            original,
        })
    }

    pub(super) fn macro_index(&self) -> Option<u32> {
        self.damage_resistance.value()?.resolved_index()
    }
}

/// One physical record defining shared damage resistance channels.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DamageResistanceMacro<'source> {
    source: CanonicalRecordHeader<'source>,
    damage_resistance: SourcedValue<MacroBody<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> DamageResistanceMacro<'source> {
    pub(super) fn read(
        _source_type: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let damage_resistance =
            fields.declared_class_to("damageResistance", INLINE, notes, MacroBody::read)?;
        let additional_fields =
            fields.authored_members_except([("damageResistance", damage_resistance.state())])?;
        Ok(Self {
            source,
            damage_resistance,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MacroBody<'source> {
    #[serde(flatten)]
    channels: ResistanceSet,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> MacroBody<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let channels = ResistanceSet::read(source, notes)?;
        let original = UnitSource::read(source, channels.projected())?;
        Ok(Self { channels, original })
    }
}
