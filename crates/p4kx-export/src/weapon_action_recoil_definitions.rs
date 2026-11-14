// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::catalog_weapon_actions::{RECOIL_DEFINITION, WeaponActions};
use super::interaction_source::original;
use super::referenced_definition_read::{self, Definitions, Selection};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// Original recoil configuration shared by action nodes in the same Build.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponActionRecoilDefinition<'source> {
    source: CanonicalRecordHeader<'source>,
    hands: SourcedValue<UnitSource<'source>>,
    aim: SourcedValue<UnitSource<'source>>,
    body: SourcedValue<UnitSource<'source>>,
    head: SourcedValue<UnitSource<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> WeaponActionRecoilDefinition<'source> {
    fn read(
        _source_type: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let hands = fields.declared_class_to(
            "weaponProceduralHandsRecoil",
            "SWeaponProceduralHandsRecoilConfigDef",
            notes,
            original,
        )?;
        let aim = fields.declared_class_to(
            "weaponProceduralAimRecoil",
            "SWeaponProceduralAimRecoilConfigDef",
            notes,
            original,
        )?;
        let body = fields.declared_class_to(
            "weaponProceduralBodyRecoil",
            "SWeaponProceduralBodyRecoilConfigDef",
            notes,
            original,
        )?;
        let head = fields.declared_class_to(
            "weaponProceduralHeadRecoil",
            "SWeaponProceduralHeadRecoilConfigDef",
            notes,
            original,
        )?;
        let additional_fields = fields.authored_members_except([
            ("weaponProceduralHandsRecoil", hands.state()),
            ("weaponProceduralAimRecoil", aim.state()),
            ("weaponProceduralBodyRecoil", body.state()),
            ("weaponProceduralHeadRecoil", head.state()),
        ])?;
        Ok(Self {
            source,
            hands,
            aim,
            body,
            head,
            additional_fields,
        })
    }
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    weapons: &[WeaponActions<'source>],
) -> Result<Definitions<'source, WeaponActionRecoilDefinition<'source>>, CatalogError> {
    let linked = weapons
        .iter()
        .flat_map(WeaponActions::linked_recoil_definitions)
        .collect();
    referenced_definition_read::read(
        forge,
        input,
        &Selection {
            source_types: &[RECOIL_DEFINITION],
            linked: &linked,
            filter,
            inspection: ConfigurationInspection::WeaponActionRecoilDefinition,
        },
        cancel,
        WeaponActionRecoilDefinition::read,
    )
}
