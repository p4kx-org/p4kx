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
use crate::catalog_queries::equipment::ship_systems::{ShieldGenerator, ShieldRange};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::resource_number::ResourceNumber;

pub(super) const PATH: &str = "configurations.shieldGenerators";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_shield_generators(CatalogRecordSelection::All, cancel, |snapshot| {
        component::stage_selected(PATH, &snapshot, scratch, cancel, Generator::read)
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Generator<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    max_health: Cell<Number<'read>>,
    max_regen: Cell<Number<'read>>,
    downed_regen_delay: Cell<Number<'read>>,
    damaged_regen_delay: Cell<Number<'read>>,
    electrical_charge_damage_resistance: Cell<Number<'read>>,
    shield_resistance: Cell<Vec<Cell<Resistance<'read>>>>,
}

impl<'read> Generator<'read> {
    fn read(source: &'read ShieldGenerator<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(source.owner(), PATH)?;
        Ok(Self {
            owner: source.owner().into(),
            max_health: quantity(source.max_health())?,
            max_regen: quantity(source.max_regen())?,
            downed_regen_delay: quantity(source.downed_regen_delay())?,
            damaged_regen_delay: quantity(source.damaged_regen_delay())?,
            electrical_charge_damage_resistance: quantity(
                source.electrical_charge_damage_resistance(),
            )?,
            shield_resistance: completeness::list(
                PATH,
                source.shield_resistance(),
                Resistance::read,
            )?,
        })
    }
}

#[derive(Serialize)]
struct Resistance<'read> {
    min: Cell<Number<'read>>,
    max: Cell<Number<'read>>,
}

impl<'read> Resistance<'read> {
    fn read(source: &'read ShieldRange<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            min: quantity(source.min())?,
            max: quantity(source.max())?,
        })
    }
}

fn quantity(source: &SourcedValue<ResourceNumber>) -> Result<Cell<Number<'_>>, ReducedError> {
    Ok(completeness::cell(PATH, source)?.map(Number::from))
}
