// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// One shop placement from the `ShopBackup` registry.
///
/// Item GUIDs come from the offline inventory file named by the shop containers (ADR-0071).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Shop {
    #[serde(flatten)]
    /// The original registry fields and their provenance.
    pub registry: ShopRegistryFields,
    /// The unique nominal Starmap GUID selected by the registry location and matching names.
    pub starmap_object_guid: SourcedValue<CatalogGuid>,
    /// The container kind supported by the matching path folders and nested `CryGUID` selectors.
    pub kind: SourcedValue<String>,
    /// The second folder below `PU\Shops`, absent when the classification has no brand.
    pub brand: SourcedValue<String>,
    /// The lower-case stem of the original selected offline inventory file (ADR-0071).
    pub inventory: SourcedValue<String>,
    /// The `EntityClassDefinition` GUIDs of the items in that inventory file, lower case, ordered
    /// and without repeats. Each GUID carries the exact contributing JSON alternatives.
    pub item_guids: SourcedValue<ShopItemGuids>,
}

/// The ordered distinct item GUIDs of the selected shop inventory.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
#[schemars(transparent)]
pub struct ShopItemGuids(
    #[schemars(with = "Vec<SourcedValue<String>>")] pub(crate) Arc<[SourcedValue<String>]>,
);

impl ShopItemGuids {
    /// Borrows the item GUIDs with their exact original JSON contributions.
    #[must_use]
    pub fn as_slice(&self) -> &[SourcedValue<String>] {
        &self.0
    }
}

impl Serialize for ShopItemGuids {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(serializer)
    }
}

/// Converted fields from one selected registry row and its owning layer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ShopRegistryFields {
    /// The identity: the `ID` chain of the registry entry, the `CryGUID`s of the containers from
    /// the system root down to the `SCShop` Entity, comma separated. Unique in the catalog.
    pub key: SourcedValue<String>,
    /// The `CryGUID` of the `SCShop` Entity, the last element of `key`. A shop module placed in
    /// several places keeps it, so it is no identity; it is the `CryGUID` the container's
    /// `entitycomponentshop.xml` names.
    pub entity_guid: SourcedValue<String>,
    /// The star system whose registry holds the shop: the platform name without the suffix
    /// `system`, e.g. `stanton`, `pyro`.
    pub system: SourcedValue<String>,
    /// The name of the `SCShop` Entity, the last element of the registry's `SuperName`, e.g.
    /// `SCShop_CommEx_TDD_NewBabbage`. An internal name, not a localisation key.
    pub entity_name: SourcedValue<String>,
    /// The `ShopName` of the registry entry, e.g. `Pharmacy_RestStop_ARC-LEO1`; absent when it is
    /// empty. An internal name, not a localisation key. Many shops share one.
    pub registry_name: SourcedValue<String>,
    /// The `DefaultValue` GUID of the registry entry; absent when it is empty. Shops with the same
    /// `registryName` share it.
    pub default_value: SourcedValue<String>,
    /// The names of the containers from the system root down to the one that holds the shop,
    /// from the `SuperName` chain without its GUIDs and without the `SCShop` Entity.
    pub containers: SourcedValue<Vec<SourcedValue<String>>>,
    /// The second container, the body or jump point the shop is below (e.g.
    /// `OOC_Stanton_4_Microtech`, `pyro5`); absent when the chain has none.
    pub body: SourcedValue<String>,
    /// The third container, the location (e.g. `Area18_City_objectContainer`); absent when the
    /// chain has none.
    pub location: SourcedValue<String>,
    /// Whether the shop is a commodity exchange or commodity terminal: `entityName` starts with
    /// `SCShop_CommEx_TDD_`, or starts with `SCShop_Platinumbay` and ends with `_Commodity`.
    pub commodity_exchange: SourcedValue<bool>,
}
