// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;

use p4kx_extraction::{SourceUses, SourcedValue};

use super::Lists;
use crate::contract::Shop;
use crate::contract_shops::ShopRegistryFields;
use crate::export_ledger::SourceDisposition;
use crate::shop_containers::{Containers, Found, Inventory};
use crate::shop_inventories::{ItemList, is_placeholder};
use crate::shop_inventory_resolution::{InventoryResolver, ShopInventoryResolution};
use crate::shop_links::Links;
use crate::shop_registry::{RegistryShop, clean, is_guid};
use crate::shop_summary::ShopSummary;
use crate::{CatalogError, ExportAccountingError};

const SYSTEM_SUFFIX: &str = "system";
const COMMEX_PREFIX: &str = "scshop_commex_tdd_";
const PLATINUMBAY_PREFIX: &str = "scshop_platinumbay";
const COMMODITY_SUFFIX: &str = "_commodity";
const LISTED_LOCATIONS: usize = 200;

pub(super) fn with_resolutions(
    registry: &[RegistryShop],
    containers: &Containers,
    links: &Links,
    lists: &Lists<'_>,
    summary: &mut ShopSummary,
    resolver: &InventoryResolver<'_, '_>,
    sources: &SourceUses,
) -> Result<(Listed, Vec<ShopInventoryResolution>), CatalogError> {
    let mut resolutions = Vec::new();
    resolutions
        .try_reserve_exact(registry.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    let listed = read(
        registry,
        containers,
        links,
        lists,
        summary,
        sources,
        |raw, placement| {
            resolutions.push(resolver.row(raw, &placement.key, &placement.entity_guid)?);
            Ok(())
        },
    )?;
    sort_resolutions(&mut resolutions);
    Ok((listed, resolutions))
}

pub(super) fn sort_resolutions(rows: &mut [ShopInventoryResolution]) {
    rows.sort_unstable_by(|left, right| left.shop_key.value().cmp(&right.shop_key.value()));
}

pub(super) struct ShopPlacement {
    pub(super) key: String,
    pub(super) entity_guid: String,
    entity_name: String,
    path: Vec<String>,
}

pub(super) fn visit_registry<'registry>(
    registry: &'registry [RegistryShop],
    summary: &mut ShopSummary,
    mut emit: impl FnMut(
        &'registry RegistryShop,
        ShopPlacement,
        &mut ShopSummary,
    ) -> Result<(), CatalogError>,
) -> Result<SourceDisposition, CatalogError> {
    let mut seen = HashSet::new();
    let mut disposition = SourceDisposition::Projected;
    for raw in registry {
        let Some(placement) = ShopPlacement::read(raw) else {
            summary.malformed += 1;
            disposition = SourceDisposition::ProjectionFailed;
            continue;
        };
        if seen.contains(&placement.key) {
            summary.duplicate += 1;
            continue;
        }
        seen.try_reserve(1)
            .map_err(CatalogError::SourceUseAllocation)?;
        seen.insert(placement.key.clone());
        emit(raw, placement, summary)?;
    }
    Ok(disposition)
}

impl ShopPlacement {
    fn read(raw: &RegistryShop) -> Option<Self> {
        let ids: Vec<String> = raw
            .id
            .text()
            .split(',')
            .map(|id| id.trim().to_ascii_lowercase())
            .collect();
        let names: Vec<&str> = raw.super_name.text().split(',').collect();
        if ids.len() != names.len() || ids.len() < 2 || ids.iter().any(|id| !is_guid(id)) {
            return None;
        }
        let (entity, parents) = names.split_last()?;
        let entity_name = clean(entity).to_owned();
        let path: Vec<String> = parents.iter().map(|name| clean(name).to_owned()).collect();
        if entity_name.is_empty() || path.iter().any(String::is_empty) {
            return None;
        }
        let entity_guid = ids.last()?.clone();
        Some(Self {
            key: ids.join(","),
            entity_guid,
            entity_name,
            path,
        })
    }
}

pub(super) struct Listed {
    pub(super) shops: Vec<Shop>,
    pub(super) disposition: SourceDisposition,
}

/// Turns the registry entries into shops, ordered by key, and counts them in `summary`.
pub(super) fn read(
    registry: &[RegistryShop],
    containers: &Containers,
    links: &Links,
    lists: &Lists<'_>,
    summary: &mut ShopSummary,
    sources: &SourceUses,
    mut emit: impl FnMut(&RegistryShop, &ShopPlacement) -> Result<(), CatalogError>,
) -> Result<Listed, CatalogError> {
    let mut built = Vec::with_capacity(registry.len());
    let mut unlinked: BTreeSet<String> = BTreeSet::new();
    let mut systems: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let disposition = visit_registry(registry, summary, |raw, placement, summary| {
        emit(raw, &placement)?;
        let shop = shop(raw, placement, containers, links, lists, sources)?;
        let entity_guid = required(&shop.registry.entity_guid)?;
        match (&shop.starmap_object_guid, shop.registry.location.value()) {
            (Some(_), _) => summary.linked += 1,
            (None, location) => {
                summary.unlinked += 1;
                if let Some(location) = location {
                    if links.is_ambiguous(location) {
                        summary.ambiguous_link += 1;
                    }
                    unlinked.insert(location.clone());
                }
            }
        }
        match containers.find(entity_guid) {
            Found::One(_) => summary.with_kind += 1,
            Found::Mixed => summary.mixed_kind += 1,
            Found::Nowhere => summary.without_container += 1,
        }
        count_inventory(&shop, entity_guid, containers, lists, summary);
        summary.commodity_exchanges += usize::from(*required(&shop.registry.commodity_exchange)?);
        *systems
            .entry(required(&shop.registry.system)?.clone())
            .or_default() += 1;
        if let Some(kind) = &shop.kind {
            *kinds.entry(kind.clone()).or_default() += 1;
        }
        built.push(shop);
        Ok(())
    })?;
    built.sort_by(|a, b| a.registry.key.value().cmp(&b.registry.key.value()));
    summary.shops = built.len();
    summary.systems = systems.into_iter().collect();
    summary.kinds = kinds.into_iter().collect();
    summary.unlinked_locations = unlinked.into_iter().take(LISTED_LOCATIONS).collect();
    Ok(Listed {
        shops: built,
        disposition,
    })
}

fn required<T>(value: &SourcedValue<T>) -> Result<&T, CatalogError> {
    value
        .value()
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn count_inventory(
    shop: &Shop,
    entity_guid: &str,
    containers: &Containers,
    lists: &Lists<'_>,
    summary: &mut ShopSummary,
) {
    match (containers.inventory(entity_guid), &shop.item_guids) {
        (_, Some(_)) => {
            summary.with_items += 1;
            summary.prefab_only_shops += usize::from(containers.only_prefab(entity_guid));
        }
        (Inventory::Ambiguous, None) => summary.inventory_ambiguous += 1,
        (Inventory::One(stem), None) if is_placeholder(stem) => {
            summary.inventory_placeholder += 1;
        }
        (Inventory::One(stem), None) => match lists.get(stem) {
            Some(ItemList::Unreadable(_)) => summary.inventory_unreadable += 1,
            Some(ItemList::EmptySource(_)) => {
                summary.inventory_unreadable += 1;
                summary.inventory_empty_source += 1;
            }
            _ => summary.inventory_missing += 1,
        },
        (Inventory::Unknown, None) => summary.inventory_unscanned += 1,
        (Inventory::None, None) => {}
    }
}

fn shop(
    raw: &RegistryShop,
    placement: ShopPlacement,
    containers: &Containers,
    links: &Links,
    lists: &Lists<'_>,
    sources: &SourceUses,
) -> Result<Shop, CatalogError> {
    let starmap_object_guid = placement.path.get(2).and_then(|name| links.find(name));
    let class = match containers.find(&placement.entity_guid) {
        Found::One(class) => Some(class),
        Found::Nowhere | Found::Mixed => None,
    };
    let (inventory, item_guids) = match containers.inventory(&placement.entity_guid) {
        Inventory::One(stem) => match lists.get(stem) {
            Some(ItemList::Items { guids, .. }) => (Some(stem.to_owned()), Some(Arc::clone(guids))),
            _ => (None, None),
        },
        Inventory::None | Inventory::Ambiguous | Inventory::Unknown => (None, None),
    };
    Ok(Shop {
        registry: registry_fields(raw, placement, sources)?,
        starmap_object_guid,
        kind: class.map(|class| class.kind.clone()),
        brand: class.and_then(|class| class.brand.clone()),
        inventory,
        item_guids,
    })
}

fn registry_fields(
    raw: &RegistryShop,
    placement: ShopPlacement,
    sources: &SourceUses,
) -> Result<ShopRegistryFields, CatalogError> {
    let ShopPlacement {
        key,
        entity_guid,
        entity_name,
        path,
    } = placement;
    let lower = entity_name.to_ascii_lowercase();
    let commodity_exchange = lower.starts_with(COMMEX_PREFIX)
        || (lower.starts_with(PLATINUMBAY_PREFIX) && lower.ends_with(COMMODITY_SUFFIX));
    let non_empty = |text: &str| (!text.trim().is_empty()).then(|| text.trim().to_owned());
    let platform = raw.platform.text().to_ascii_lowercase();
    let system = platform
        .strip_suffix(SYSTEM_SUFFIX)
        .unwrap_or(&platform)
        .to_owned();
    let names = &raw.super_name;
    let body = names.value(sources, path.get(1).cloned())?;
    let location = names.value(sources, path.get(2).cloned())?;
    let containers = path
        .into_iter()
        .map(|name| names.value(sources, Some(name)))
        .collect::<Result<Vec<_>, CatalogError>>()?;
    Ok(ShopRegistryFields {
        key: raw.id_origin.value(sources, Some(key))?,
        entity_guid: raw.id_origin.value(sources, Some(entity_guid))?,
        system: raw.platform.value(sources, Some(system))?,
        entity_name: names.value(sources, Some(entity_name))?,
        registry_name: raw
            .shop_name
            .value(sources, non_empty(raw.shop_name.text()))?,
        default_value: raw
            .default_value
            .value(sources, non_empty(raw.default_value.text()))?,
        containers: names.value(sources, Some(containers))?,
        body,
        location,
        commodity_exchange: names.value(sources, Some(commodity_exchange))?,
    })
}
