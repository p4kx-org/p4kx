// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::number::Number;
use crate::catalog_queries::equipment::EquipmentSnapshot;
use crate::catalog_queries::equipment::worn_gear::{Clothing, ItemRadar};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::resource_float::ResourceFloat32;

pub(super) const CLOTHING: &str = "configurations.clothing";
pub(super) const RADARS: &str = "configurations.itemRadars";

pub(super) struct WearableFields {
    pub(super) clothing: StagedCatalogTable,
    pub(super) radars: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<WearableFields, ReducedError> {
    let clothing = parsed.with_clothing(CatalogRecordSelection::All, cancel, |snapshot| {
        stage(CLOTHING, &snapshot, scratch, cancel, Protection::read)
    })??;
    let radars = parsed.with_item_radars(CatalogRecordSelection::All, cancel, |snapshot| {
        stage(RADARS, &snapshot, scratch, cancel, Radar::read)
    })??;
    Ok(WearableFields { clothing, radars })
}

fn stage<'read, Source, Reduced: Serialize>(
    path: &'static str,
    snapshot: &'read EquipmentSnapshot<'_, Source>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    mut project: impl FnMut(&'read Source) -> Result<Reduced, ReducedError>,
) -> Result<StagedCatalogTable, ReducedError> {
    let availability = snapshot.availability();
    completeness::inventory(path, availability, snapshot.coverage())?;
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for row in snapshot.rows() {
        output.append(&project(row)?)?;
    }
    let table = output.finish(availability)?;
    completeness::rows(path, snapshot.rows().len(), table.len())?;
    Ok(table)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Protection<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    temperature_resistance: Cell<Temperature>,
    radiation_resistance: Cell<Radiation>,
}

impl<'read> Protection<'read> {
    fn read(source: &'read Clothing<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(source.owner(), CLOTHING)?;
        Ok(Self {
            owner: source.owner().into(),
            temperature_resistance: completeness::cell(CLOTHING, source.temperature_resistance())?
                .try_map(|temperature| {
                    Ok::<_, ReducedError>(Temperature {
                        min_resistance: number(CLOTHING, temperature.min_resistance())?,
                        max_resistance: number(CLOTHING, temperature.max_resistance())?,
                    })
                })?,
            radiation_resistance: completeness::cell(CLOTHING, source.radiation_resistance())?
                .try_map(|radiation| {
                    Ok::<_, ReducedError>(Radiation {
                        maximum_radiation_capacity: number(
                            CLOTHING,
                            radiation.maximum_radiation_capacity(),
                        )?,
                        radiation_dissipation_rate: number(
                            CLOTHING,
                            radiation.radiation_dissipation_rate(),
                        )?,
                    })
                })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Temperature {
    min_resistance: Cell<Number<'static>>,
    max_resistance: Cell<Number<'static>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Radiation {
    maximum_radiation_capacity: Cell<Number<'static>>,
    radiation_dissipation_rate: Cell<Number<'static>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Radar<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    aim_assist: Cell<AssignmentRange>,
}

impl<'read> Radar<'read> {
    fn read(source: &'read ItemRadar<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(source.owner(), RADARS)?;
        Ok(Self {
            owner: source.owner().into(),
            aim_assist: completeness::cell(RADARS, source.aim_assist())?.try_map(|aim| {
                Ok::<_, ReducedError>(AssignmentRange {
                    distance_min_assignment: number(RADARS, aim.distance_min_assignment())?,
                    distance_max_assignment: number(RADARS, aim.distance_max_assignment())?,
                })
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignmentRange {
    distance_min_assignment: Cell<Number<'static>>,
    distance_max_assignment: Cell<Number<'static>>,
}

fn number(
    path: &'static str,
    value: &SourcedValue<ResourceFloat32>,
) -> Result<Cell<Number<'static>>, ReducedError> {
    Ok(completeness::cell(path, value)?.map(Number::from))
}
