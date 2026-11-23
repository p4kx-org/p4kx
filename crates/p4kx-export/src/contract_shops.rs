// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::{Serialize, Serializer};

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// A shop: one `ShopBackup` entry of the shop registry `Data\Libs\CosmoTools\LayerRecovery\
/// LayerBackups.xml`, one placement of an `SCShop` Entity in the world. No prices and no stock; the
/// item GUIDs come only from an inventory file that the shop containers name (ADR-0071).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Shop {
    #[serde(flatten)]
    pub(crate) registry: ShopRegistryFields,
    /// The `guid` of the `starmapObjects` entry whose `entityName` or `className` equals
    /// `location` ignoring ASCII case; `null` when none does or more than one object does.
    pub(crate) starmap_object_guid: Option<CatalogGuid>,
    /// The kind from the path of the shop container whose `SCShop` Entity is `entityGuid`:
    /// the folder below `PU\Shops` (e.g. `commex`, `clothing`), or `shopcomm` or `shopfood` for a
    /// container below `PU\loc`. A closed set the game may extend; `null` when no container or
    /// containers of different kinds hold the Entity.
    pub(crate) kind: Option<String>,
    /// The brand from the same path: the second folder below `PU\Shops` (e.g. `tdd`, `mcraes`);
    /// `null` for a container below `PU\loc`, one without a brand folder, or when the containers
    /// disagree.
    pub(crate) brand: Option<String>,
    /// The lower-case file stem of `Data\Scripts\ShopInventories\<stem>.json` that the `.soc` of
    /// every shop container naming `entityGuid` gives the `SCShop` component (ADR-0071); `null`
    /// when no container names a file, the containers disagree or the file is missing or
    /// unreadable.
    pub(crate) inventory: Option<String>,
    /// The `EntityClassDefinition` GUIDs of the items in that inventory file, lower case, ordered
    /// and without repeats; `null` exactly when `inventory` is. No price, stock or rental data.
    #[serde(serialize_with = "serialize_item_guids")]
    #[schemars(with = "Option<Vec<String>>")]
    pub(crate) item_guids: Option<Arc<[String]>>,
}

/// Converted fields from one selected registry row and its owning layer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShopRegistryFields {
    /// The identity: the `ID` chain of the registry entry, the `CryGUID`s of the containers from
    /// the system root down to the `SCShop` Entity, comma separated. Unique in the catalog.
    pub(crate) key: SourcedValue<String>,
    /// The `CryGUID` of the `SCShop` Entity, the last element of `key`. A shop module placed in
    /// several places keeps it, so it is no identity; it is the `CryGUID` the container's
    /// `entitycomponentshop.xml` names.
    pub(crate) entity_guid: SourcedValue<String>,
    /// The star system whose registry holds the shop: the platform name without the suffix
    /// `system`, e.g. `stanton`, `pyro`.
    pub(crate) system: SourcedValue<String>,
    /// The name of the `SCShop` Entity, the last element of the registry's `SuperName`, e.g.
    /// `SCShop_CommEx_TDD_NewBabbage`. An internal name, not a localisation key.
    pub(crate) entity_name: SourcedValue<String>,
    /// The `ShopName` of the registry entry, e.g. `Pharmacy_RestStop_ARC-LEO1`; absent when it is
    /// empty. An internal name, not a localisation key. Many shops share one.
    pub(crate) registry_name: SourcedValue<String>,
    /// The `DefaultValue` GUID of the registry entry; absent when it is empty. Shops with the same
    /// `registryName` share it.
    pub(crate) default_value: SourcedValue<String>,
    /// The names of the containers from the system root down to the one that holds the shop,
    /// from the `SuperName` chain without its GUIDs and without the `SCShop` Entity.
    pub(crate) containers: SourcedValue<Vec<SourcedValue<String>>>,
    /// The second container, the body or jump point the shop is below (e.g.
    /// `OOC_Stanton_4_Microtech`, `pyro5`); absent when the chain has none.
    pub(crate) body: SourcedValue<String>,
    /// The third container, the location (e.g. `Area18_City_objectContainer`); absent when the
    /// chain has none.
    pub(crate) location: SourcedValue<String>,
    /// Whether the shop is a commodity exchange or commodity terminal: `entityName` starts with
    /// `SCShop_CommEx_TDD_`, or starts with `SCShop_Platinumbay` and ends with `_Commodity`.
    pub(crate) commodity_exchange: SourcedValue<bool>,
}

#[expect(clippy::ref_option, reason = "serde passes the field by reference")]
fn serialize_item_guids<S: Serializer>(
    guids: &Option<Arc<[String]>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    guids.as_deref().serialize(serializer)
}
