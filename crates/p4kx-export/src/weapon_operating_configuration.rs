// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::interaction_source::{RecordLink, original};
use crate::catalog_v3::weapon_procedural_definitions::{
    ANIMATION_SOURCE_TYPE, RECOIL_CONFIG_SOURCE_TYPE,
};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const WEAPON_STATS: &str = "SWeaponStats";

/// Authored Record links and operating modifiers of the physical weapon component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WeaponOperatingConfiguration<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    procedural_animation_record: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    actor_procedural_recoil_config: RecordLink<'source>,
    connection_params: SourcedValue<WeaponConnection<'source>>,
    weapon_degradation_modifier: SourcedValue<WeaponModifier<'source>>,
}

impl<'source> WeaponOperatingConfiguration<'source> {
    pub(super) fn animation_index(&self) -> Option<u32> {
        self.procedural_animation_record.value()?.resolved_index()
    }

    pub(super) fn actor_recoil_index(&self) -> Option<u32> {
        self.actor_procedural_recoil_config
            .value()?
            .resolved_index()
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            procedural_animation_record: source.record_link_to(
                "proceduralAnimationRecord",
                ANIMATION_SOURCE_TYPE,
                notes,
            )?,
            actor_procedural_recoil_config: source.record_link_to(
                "actorProceduralRecoilConfig",
                RECOIL_CONFIG_SOURCE_TYPE,
                notes,
            )?,
            connection_params: source.declared_class_to(
                "connectionParams",
                "SWeaponConnectionParams",
                notes,
                WeaponConnection::read,
            )?,
            weapon_degradation_modifier: WeaponModifier::read_field(
                source,
                "weaponDegradationModifier",
                notes,
            )?,
        })
    }
}

/// Stored modifier blocks for each power and temperature operating state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct WeaponConnection<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    no_power_stats: SourcedValue<UnitSource<'source>>,
    underpower_stats: SourcedValue<UnitSource<'source>>,
    overpower_stats: SourcedValue<UnitSource<'source>>,
    overclock_stats: SourcedValue<UnitSource<'source>>,
    heat_stats: SourcedValue<UnitSource<'source>>,
    overheat_locked_stats: SourcedValue<UnitSource<'source>>,
}

impl<'source> WeaponConnection<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let no_power_stats = read_stats(source, "noPowerStats", notes)?;
        let underpower_stats = read_stats(source, "underpowerStats", notes)?;
        let overpower_stats = read_stats(source, "overpowerStats", notes)?;
        let overclock_stats = read_stats(source, "overclockStats", notes)?;
        let heat_stats = read_stats(source, "heatStats", notes)?;
        let overheat_locked_stats = source.strong_pointer_family_to(
            "overheatLockedStats",
            WEAPON_STATS,
            notes,
            original,
        )?;
        Ok(Self {
            source: UnitSource::read(
                source,
                [
                    ("noPowerStats", no_power_stats.state()),
                    ("underpowerStats", underpower_stats.state()),
                    ("overpowerStats", overpower_stats.state()),
                    ("overclockStats", overclock_stats.state()),
                    ("heatStats", heat_stats.state()),
                    ("overheatLockedStats", overheat_locked_stats.state()),
                ],
            )?,
            no_power_stats,
            underpower_stats,
            overpower_stats,
            overclock_stats,
            heat_stats,
            overheat_locked_stats,
        })
    }
}

/// A concrete modifier and its independently authored weapon stats.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct WeaponModifier<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    weapon_stats: SourcedValue<UnitSource<'source>>,
}

impl<'source> WeaponModifier<'source> {
    pub(super) fn read_field(
        source: Observed<'_, '_, '_, 'source>,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        source.strong_pointer_family_to(name, "SWeaponModifierParams", notes, Self::read)
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let weapon_stats = read_stats(source, "weaponStats", notes)?;
        Ok(Self {
            source: UnitSource::read(source, [("weaponStats", weapon_stats.state())])?,
            weapon_stats,
        })
    }
}

fn read_stats<'source>(
    source: Observed<'_, '_, '_, 'source>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<UnitSource<'source>>, CatalogError> {
    source.declared_class_to(name, WEAPON_STATS, notes, original)
}
