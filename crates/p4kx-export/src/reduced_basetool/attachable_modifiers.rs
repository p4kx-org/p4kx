// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{self, Component, ComponentKey};
use super::number::Number;
use crate::catalog_queries::equipment::mining_tools::{
    AttachableEffect, AttachableModifier, SalvageModifier, WeaponEffect, WeaponModifier,
    WeaponModifierStats,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::resource_float::ResourceFloat32;

pub(super) const PATH: &str = "configurations.attachableModifiers";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_attachable_modifiers(CatalogRecordSelection::All, cancel, |snapshot| {
        component::stage_selected(PATH, &snapshot, scratch, cancel, Attachment::read)
    })?
}

#[derive(Serialize)]
struct Attachment<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    modifiers: Cell<Vec<Cell<Effect>>>,
}

impl<'read> Attachment<'read> {
    fn read(source: &'read AttachableModifier<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(source.owner(), PATH)?;
        Ok(Self {
            owner: source.owner().into(),
            modifiers: completeness::list(PATH, source.modifiers(), Effect::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum Effect {
    MiningLaser,
    MineableRock,
    MiningFilter,
    Weapon(Weapon),
    Authored,
}

impl Effect {
    fn read(source: &AttachableEffect<'_>) -> Result<Self, ReducedError> {
        Ok(match source {
            AttachableEffect::MiningLaser(_) => Self::MiningLaser,
            AttachableEffect::MineableRock(_) => Self::MineableRock,
            AttachableEffect::MiningFilter(_) => Self::MiningFilter,
            AttachableEffect::Weapon(weapon) => Self::Weapon(Weapon::read(weapon)?),
            AttachableEffect::Authored(_) => Self::Authored,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Weapon {
    weapon_modifier: Cell<Modifier>,
}

impl Weapon {
    fn read(source: &WeaponEffect<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            weapon_modifier: completeness::cell(PATH, source.weapon_modifier())?
                .try_map(Modifier::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Modifier {
    weapon_stats: Cell<WeaponStats>,
}

impl Modifier {
    fn read(source: &WeaponModifier<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            weapon_stats: completeness::cell(PATH, source.weapon_stats())?
                .try_map(WeaponStats::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponStats {
    salvage_modifier: Cell<SalvageMultipliers>,
}

impl WeaponStats {
    fn read(source: &WeaponModifierStats<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            salvage_modifier: completeness::cell(PATH, source.salvage_modifier())?
                .try_map(SalvageMultipliers::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SalvageMultipliers {
    radius_multiplier: Cell<Number<'static>>,
    salvage_speed_multiplier: Cell<Number<'static>>,
    extraction_efficiency: Cell<Number<'static>>,
}

impl SalvageMultipliers {
    fn read(source: &SalvageModifier<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            radius_multiplier: single(source.radius_multiplier())?,
            salvage_speed_multiplier: single(source.salvage_speed_multiplier())?,
            extraction_efficiency: single(source.extraction_efficiency())?,
        })
    }
}

fn single(source: &SourcedValue<ResourceFloat32>) -> Result<Cell<Number<'static>>, ReducedError> {
    Ok(completeness::cell(PATH, source)?.map(Number::from))
}
