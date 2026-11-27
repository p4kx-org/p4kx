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
use super::record_targets::RecordTargets;
use crate::catalog_queries::equipment::resource_network::{
    PowerRange, ResourceComponent, ResourcePowerRanges, ResourceState,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::resource_float::ResourceFloat32;

#[path = "resource_component_deltas.rs"]
mod deltas;

use self::deltas::Delta;

pub(super) const PATH: &str = "configurations.resourceComponents";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    resource_targets: &RecordTargets,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_resource_components(CatalogRecordSelection::All, cancel, |snapshot| {
        component::stage_selected(PATH, &snapshot, scratch, cancel, |row| {
            Resource::read(row, resource_targets)
        })
    })?
}

#[derive(Serialize)]
struct Resource<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    states: Cell<Vec<Cell<State<'read>>>>,
}

impl<'read> Resource<'read> {
    fn read(
        original: &'read ResourceComponent<'_>,
        resource_targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        ComponentKey::read(original.owner(), PATH)?;
        Ok(Self {
            owner: original.owner().into(),
            states: completeness::list(PATH, &original.states, |state| {
                State::read(state, resource_targets)
            })?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct State<'read> {
    source_position: Cell<&'read u32>,
    name: Cell<&'read str>,
    deltas: Cell<Vec<Cell<Delta<'read>>>>,
    power_ranges: Cell<Ranges<'read>>,
}

impl<'read> State<'read> {
    fn read(
        original: &'read ResourceState<'_>,
        resource_targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            source_position: completeness::cell(PATH, &original.source_position)?,
            name: text(&original.name)?,
            deltas: completeness::list(PATH, &original.deltas, |delta| {
                Delta::read(delta, resource_targets)
            })?,
            power_ranges: completeness::cell(PATH, &original.power_ranges)?
                .try_map(Ranges::read)?,
        })
    }
}

#[derive(Serialize)]
struct Ranges<'read> {
    low: Cell<CapacityRange<'read>>,
    medium: Cell<CapacityRange<'read>>,
    high: Cell<CapacityRange<'read>>,
}

impl<'read> Ranges<'read> {
    fn read(original: &'read ResourcePowerRanges<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            low: completeness::cell(PATH, &original.low)?.try_map(CapacityRange::read)?,
            medium: completeness::cell(PATH, &original.medium)?.try_map(CapacityRange::read)?,
            high: completeness::cell(PATH, &original.high)?.try_map(CapacityRange::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapacityRange<'read> {
    start: Cell<&'read i32>,
    modifier: Cell<Number<'static>>,
    register_range: Cell<&'read bool>,
}

impl<'read> CapacityRange<'read> {
    fn read(original: &'read PowerRange<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            start: completeness::cell(PATH, &original.start)?,
            modifier: single(&original.modifier)?,
            register_range: completeness::cell(PATH, &original.register_range)?,
        })
    }
}

fn single(original: &SourcedValue<ResourceFloat32>) -> Result<Cell<Number<'static>>, ReducedError> {
    Ok(completeness::cell(PATH, original)?.map(Number::from))
}

fn text<'read>(original: &'read SourcedValue<&str>) -> Result<Cell<&'read str>, ReducedError> {
    Ok(completeness::cell(PATH, original)?.map(|value| *value))
}
