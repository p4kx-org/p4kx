// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use crate::catalog_queries::equipment::cargo_globals::{
    CargoBoxSizes, CargoLoadingGlobal, ShopCommodityGlobal,
};
use crate::catalog_queries::{
    CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability, ParsedCatalogQueries,
};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::resource_float::ResourceFloat32;

pub(super) const LOADING: &str = "configurations.cargoLoadingGlobals";
pub(super) const COMMODITY: &str = "configurations.shopCommodityGlobals";

pub(super) struct CargoGlobals {
    pub(super) loading: StagedCatalogTable,
    pub(super) commodity: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<CargoGlobals, ReducedError> {
    let loading =
        parsed.with_cargo_loading_globals(CatalogRecordSelection::All, cancel, |input| {
            stage(
                LOADING,
                scratch,
                cancel,
                input.availability(),
                input.coverage(),
                input.rows().iter().map(Loading::from),
            )
        })??;
    let commodity =
        parsed.with_shop_commodity_globals(CatalogRecordSelection::All, cancel, |input| {
            stage(
                COMMODITY,
                scratch,
                cancel,
                input.availability(),
                input.coverage(),
                input.rows().iter().map(Commodity::from),
            )
        })??;
    Ok(CargoGlobals { loading, commodity })
}

fn stage(
    path: &'static str,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    availability: DatasetAvailability,
    coverage: &CatalogRecordCoverage,
    rows: impl ExactSizeIterator<Item = impl Serialize>,
) -> Result<StagedCatalogTable, ReducedError> {
    completeness::all_fields(path, availability, coverage)?;
    let expected = rows.len();
    let mut staged = StagedCatalogRows::new(scratch, cancel);
    for row in rows {
        staged.append(&row)?;
    }
    let table = staged.finish(availability)?;
    completeness::rows(path, expected, table.len())?;
    Ok(table)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Loading<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    auto_loading_base_loading_time: Cell<Number<'static>>,
    auto_loading_base_unloading_time: Cell<Number<'static>>,
    #[serde(rename = "cargoDeckLoadingTimePerSCU")]
    cargo_deck_loading_time_per_scu: Cell<Number<'static>>,
    auto_loading_box_size_loading_time: Cell<BoxSizes<'read, Number<'static>>>,
    auto_loading_box_size_unloading_time: Cell<BoxSizes<'read, Number<'static>>>,
}

impl<'read> From<&'read CargoLoadingGlobal<'_>> for Loading<'read> {
    fn from(value: &'read CargoLoadingGlobal<'_>) -> Self {
        Self {
            identity: value.source().into(),
            auto_loading_base_loading_time: time(value.auto_loading_base_loading_time()),
            auto_loading_base_unloading_time: time(value.auto_loading_base_unloading_time()),
            cargo_deck_loading_time_per_scu: time(value.cargo_deck_loading_time_per_scu()),
            auto_loading_box_size_loading_time: box_sizes(
                value.auto_loading_box_size_loading_time(),
                time,
            ),
            auto_loading_box_size_unloading_time: box_sizes(
                value.auto_loading_box_size_unloading_time(),
                time,
            ),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Commodity<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    auto_loading_base_price: Cell<&'read i32>,
    auto_loading_box_size_prices: Cell<BoxSizes<'read, &'read i32>>,
}

impl<'read> From<&'read ShopCommodityGlobal<'_>> for Commodity<'read> {
    fn from(value: &'read ShopCommodityGlobal<'_>) -> Self {
        Self {
            identity: value.source().into(),
            auto_loading_base_price: value.auto_loading_base_price().into(),
            auto_loading_box_size_prices: box_sizes(
                value.auto_loading_box_size_prices(),
                Cell::from,
            ),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BoxSizes<'read, T> {
    source_type: Cell<&'read str>,
    by_scu: BTreeMap<&'static str, Cell<T>>,
}

fn box_sizes<'read, T, U>(
    value: &'read SourcedValue<CargoBoxSizes<'_, T>>,
    mut project: impl FnMut(&'read SourcedValue<T>) -> Cell<U>,
) -> Cell<BoxSizes<'read, U>> {
    Cell::from(value).map(|sizes| BoxSizes {
        source_type: Cell::from(sizes.source_type()).map(|name| *name),
        by_scu: sizes
            .by_scu()
            .iter()
            .map(|(&scu, value)| (scu, project(value)))
            .collect(),
    })
}

fn time(value: &SourcedValue<ResourceFloat32>) -> Cell<Number<'static>> {
    Cell::from(value).map(Number::from)
}
