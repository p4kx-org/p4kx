// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::cargo_globals;

use crate::record_observation::RecordObservation;

pub use cargo_globals::{CargoBoxSizes, CargoLoadingGlobal, ShopCommodityGlobal};

struct CargoLoadingRows;

impl Projector for CargoLoadingRows {
    type Row<'source> = CargoLoadingGlobal<'source>;
    const FAMILY: Family = Family::CargoLoadingGlobals;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = cargo_globals::read_loading(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row.into_definition()]))
    }
}

struct ShopCommodityRows;

impl Projector for ShopCommodityRows {
    type Row<'source> = ShopCommodityGlobal<'source>;
    const FAMILY: Family = Family::ShopCommodityGlobals;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = cargo_globals::read_shop(tree, projection, admission, cancel)?;
        Ok(read.map(|row| vec![row.into_definition()]))
    }
}

record_query::unlocalized!(
    CargoLoadingRows,
    CargoLoadingGlobal,
    /// Authored loading times and their original source evidence.
    CargoLoadingGlobalSnapshot,
    with_cargo_loading_globals,
    with_cargo_loading_globals_from
);

record_query::unlocalized!(
    ShopCommodityRows,
    ShopCommodityGlobal,
    /// Authored commodity loading fees and their original source evidence.
    ShopCommodityGlobalSnapshot,
    with_shop_commodity_globals,
    with_shop_commodity_globals_from
);
