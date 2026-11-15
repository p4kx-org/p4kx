// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_table::CatalogTable;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::selection_rules::KindRule;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "cargo_box_sizes.rs"]
mod cargo_box_sizes;

use cargo_box_sizes::CargoBoxSizes;

const LOADING_TYPE: &str = "GlobalCargoLoadingParams";
const SHOP_TYPE: &str = "GlobalShopCommodityParams";

pub(crate) const LOADING_RULE: KindRule = KindRule {
    prefixes: &[""],
    type_name: LOADING_TYPE,
};
pub(crate) const SHOP_RULE: KindRule = KindRule {
    prefixes: &[""],
    type_name: SHOP_TYPE,
};
const BOX_TIMES_TYPE: &str = "SAutoLoadingBoxSizeLoadingTime";
const BOX_PRICES_TYPE: &str = "SAutoLoadingBoxSizePrices";

const BASE_LOADING: &str = "autoLoadingBaseLoadingTime";
const BASE_UNLOADING: &str = "autoLoadingBaseUnloadingTime";
const CARGO_DECK: &str = "cargoDeckLoadingTimePerSCU";
const BOX_LOADING: &str = "autoLoadingBoxSizeLoadingTime";
const BOX_UNLOADING: &str = "autoLoadingBoxSizeUnloadingTime";
const BASE_PRICE: &str = "autoLoadingBasePrice";
const BOX_PRICES: &str = "autoLoadingBoxSizePrices";

/// The authored time inputs for automated and cargo-deck loading.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CargoLoadingGlobal<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    auto_loading_base_loading_time: SourcedValue<ResourceFloat32>,
    auto_loading_base_unloading_time: SourcedValue<ResourceFloat32>,
    #[serde(rename = "cargoDeckLoadingTimePerSCU")]
    cargo_deck_loading_time_per_scu: SourcedValue<ResourceFloat32>,
    auto_loading_box_size_loading_time: SourcedValue<CargoBoxSizes<'source, ResourceFloat32>>,
    auto_loading_box_size_unloading_time: SourcedValue<CargoBoxSizes<'source, ResourceFloat32>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> CargoLoadingGlobal<'source> {
    fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let auto_loading_base_loading_time = fields.single(BASE_LOADING, notes)?;
        let auto_loading_base_unloading_time = fields.single(BASE_UNLOADING, notes)?;
        let cargo_deck_loading_time_per_scu = fields.single(CARGO_DECK, notes)?;
        let box_times = |field, notes: &mut Vec<_>| {
            fields.declared_class_to(field, BOX_TIMES_TYPE, notes, |sizes, notes| {
                CargoBoxSizes::read(sizes, notes, |source, member, notes| {
                    source.single(member, notes)
                })
            })
        };
        let auto_loading_box_size_loading_time = box_times(BOX_LOADING, notes)?;
        let auto_loading_box_size_unloading_time = box_times(BOX_UNLOADING, notes)?;
        let additional_fields = fields.authored_members_except([
            (BASE_LOADING, auto_loading_base_loading_time.state()),
            (BASE_UNLOADING, auto_loading_base_unloading_time.state()),
            (CARGO_DECK, cargo_deck_loading_time_per_scu.state()),
            (BOX_LOADING, auto_loading_box_size_loading_time.state()),
            (BOX_UNLOADING, auto_loading_box_size_unloading_time.state()),
        ])?;
        Ok(Self {
            source,
            auto_loading_base_loading_time,
            auto_loading_base_unloading_time,
            cargo_deck_loading_time_per_scu,
            auto_loading_box_size_loading_time,
            auto_loading_box_size_unloading_time,
            additional_fields,
        })
    }
}

/// The authored fee inputs for automated commodity loading.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShopCommodityGlobal<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    auto_loading_base_price: SourcedValue<i32>,
    auto_loading_box_size_prices: SourcedValue<CargoBoxSizes<'source, i32>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ShopCommodityGlobal<'source> {
    fn project(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let auto_loading_base_price = fields.int32(BASE_PRICE, notes)?;
        let auto_loading_box_size_prices =
            fields.declared_class_to(BOX_PRICES, BOX_PRICES_TYPE, notes, |sizes, notes| {
                CargoBoxSizes::read(sizes, notes, |source, member, notes| {
                    source.int32(member, notes)
                })
            })?;
        let additional_fields = fields.authored_members_except([
            (BASE_PRICE, auto_loading_base_price.state()),
            (BOX_PRICES, auto_loading_box_size_prices.state()),
        ])?;
        Ok(Self {
            source,
            auto_loading_base_price,
            auto_loading_box_size_prices,
            additional_fields,
        })
    }
}

pub(crate) type PreparedCargoLoadingGlobal<'source> =
    PreparedDefinition<'source, CargoLoadingGlobal<'source>>;
pub(crate) type PreparedShopCommodityGlobal<'source> =
    PreparedDefinition<'source, ShopCommodityGlobal<'source>>;

#[derive(Default)]
pub(crate) struct CargoGlobals<'source> {
    pub(crate) loading: Vec<PreparedCargoLoadingGlobal<'source>>,
    pub(crate) shop: Vec<PreparedShopCommodityGlobal<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CargoGlobalTables<'source> {
    pub(crate) cargo_loading_globals: CatalogTable<CargoLoadingGlobal<'source>>,
    pub(crate) shop_commodity_globals: CatalogTable<ShopCommodityGlobal<'source>>,
}

impl<'source> CargoGlobals<'source> {
    pub(crate) fn into_tables(mut self, forge: &DataForge<'_>) -> CargoGlobalTables<'source> {
        PreparedDefinition::order(&mut self.loading);
        PreparedDefinition::order(&mut self.shop);
        CargoGlobalTables {
            cargo_loading_globals: PreparedDefinition::table(forge, LOADING_TYPE, self.loading),
            shop_commodity_globals: PreparedDefinition::table(forge, SHOP_TYPE, self.shop),
        }
    }

    pub(crate) fn sources<'rows>(
        &'rows self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
        PreparedDefinition::sources(Kind::CargoLoadingGlobals, &self.loading).chain(
            PreparedDefinition::sources(Kind::ShopCommodityGlobals, &self.shop),
        )
    }
}

pub(crate) fn read<'source>(
    kind: Kind,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(Picked<'source>, Vec<FieldError>), CatalogError> {
    match (kind, tree.type_name()) {
        (Kind::CargoLoadingGlobals, LOADING_TYPE) => {
            let (row, notes) = PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| CargoLoadingGlobal::project(source, fields, notes),
            )?;
            Ok((Picked::CargoLoadingGlobal(Box::new(row)), notes))
        }
        (Kind::ShopCommodityGlobals, SHOP_TYPE) => {
            let (row, notes) = PreparedDefinition::read(
                tree,
                projection,
                admission,
                cancel,
                |source, fields, _, notes| ShopCommodityGlobal::project(source, fields, notes),
            )?;
            Ok((Picked::ShopCommodityGlobal(Box::new(row)), notes))
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

pub(crate) fn kind_for_type(type_name: &str) -> Option<Kind> {
    match type_name {
        LOADING_TYPE => Some(Kind::CargoLoadingGlobals),
        SHOP_TYPE => Some(Kind::ShopCommodityGlobals),
        _ => None,
    }
}
