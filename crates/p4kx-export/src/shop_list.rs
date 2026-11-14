// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::Lists;
use crate::CatalogError;
use crate::contract::Shop;
use crate::export_ledger::SourceDisposition;
use crate::shop_containers::{Containers, Found, Inventory};
use crate::shop_inventories::{ItemList, is_placeholder};
use crate::shop_inventory_resolution::{InventoryResolver, ShopInventoryResolution};
use crate::shop_links::Links;
use crate::shop_registry::{RegistryShop, clean, is_guid};
use crate::shop_summary::ShopSummary;

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
) -> Result<(Listed, Vec<ShopInventoryResolution>), CatalogError> {
    let mut resolutions = Vec::new();
    resolutions
        .try_reserve_exact(registry.len())
        .map_err(CatalogError::SourceUseAllocation)?;
    let listed = read(registry, containers, links, lists, summary, |raw, shop| {
        resolutions.push(resolver.row(raw, shop)?);
        Ok(())
    })?;
    resolutions.sort_unstable_by(|left, right| left.shop_key.value().cmp(&right.shop_key.value()));
    Ok((listed, resolutions))
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
    mut emit: impl FnMut(&RegistryShop, &Shop) -> Result<(), CatalogError>,
) -> Result<Listed, CatalogError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut built = Vec::with_capacity(registry.len());
    let mut unlinked: BTreeSet<String> = BTreeSet::new();
    let mut systems: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut disposition = SourceDisposition::Projected;
    for raw in registry {
        let Some(shop) = shop(raw, containers, links, lists) else {
            summary.malformed += 1;
            disposition = SourceDisposition::ProjectionFailed;
            continue;
        };
        if !seen.insert(shop.key.clone()) {
            summary.duplicate += 1;
            continue;
        }
        emit(raw, &shop)?;
        match (&shop.starmap_object_guid, &shop.location) {
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
        match containers.find(&shop.entity_guid) {
            Found::One(_) => summary.with_kind += 1,
            Found::Mixed => summary.mixed_kind += 1,
            Found::Nowhere => summary.without_container += 1,
        }
        count_inventory(&shop, containers, lists, summary);
        summary.commodity_exchanges += usize::from(shop.commodity_exchange);
        *systems.entry(shop.system.clone()).or_default() += 1;
        if let Some(kind) = &shop.kind {
            *kinds.entry(kind.clone()).or_default() += 1;
        }
        built.push(shop);
    }
    built.sort_by(|a, b| a.key.cmp(&b.key));
    summary.shops = built.len();
    summary.systems = systems.into_iter().collect();
    summary.kinds = kinds.into_iter().collect();
    summary.unlinked_locations = unlinked.into_iter().take(LISTED_LOCATIONS).collect();
    Ok(Listed {
        shops: built,
        disposition,
    })
}

fn count_inventory(
    shop: &Shop,
    containers: &Containers,
    lists: &Lists<'_>,
    summary: &mut ShopSummary,
) {
    match (containers.inventory(&shop.entity_guid), &shop.item_guids) {
        (_, Some(_)) => {
            summary.with_items += 1;
            summary.prefab_only_shops += usize::from(containers.only_prefab(&shop.entity_guid));
        }
        (Inventory::Ambiguous, None) => summary.inventory_ambiguous += 1,
        (Inventory::One(stem), None) if is_placeholder(stem) => {
            summary.inventory_placeholder += 1;
        }
        (Inventory::One(stem), None) => match lists.get(stem) {
            Some(ItemList::Unreadable(_)) => summary.inventory_unreadable += 1,
            _ => summary.inventory_missing += 1,
        },
        (Inventory::Unknown, None) => summary.inventory_unscanned += 1,
        (Inventory::None, None) => {}
    }
}

fn shop(
    raw: &RegistryShop,
    containers: &Containers,
    links: &Links,
    lists: &Lists<'_>,
) -> Option<Shop> {
    let ids: Vec<String> = raw
        .id
        .text()
        .split(',')
        .map(|id| id.trim().to_ascii_lowercase())
        .collect();
    let names: Vec<&str> = raw.super_name.split(',').collect();
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
    let location = path.get(2).cloned();
    let starmap_object_guid = location.as_deref().and_then(|name| links.find(name));
    let class = match containers.find(&entity_guid) {
        Found::One(class) => Some(class),
        Found::Nowhere | Found::Mixed => None,
    };
    let lower = entity_name.to_ascii_lowercase();
    let commodity_exchange = lower.starts_with(COMMEX_PREFIX)
        || (lower.starts_with(PLATINUMBAY_PREFIX) && lower.ends_with(COMMODITY_SUFFIX));
    let (inventory, item_guids) = match containers.inventory(&entity_guid) {
        Inventory::One(stem) => match lists.get(stem) {
            Some(ItemList::Items { guids, .. }) => (Some(stem.to_owned()), Some(guids.clone())),
            _ => (None, None),
        },
        Inventory::None | Inventory::Ambiguous | Inventory::Unknown => (None, None),
    };
    let non_empty = |text: &str| (!text.trim().is_empty()).then(|| text.trim().to_owned());
    let platform = raw.platform.to_ascii_lowercase();
    Some(Shop {
        key: ids.join(","),
        entity_guid,
        system: platform
            .strip_suffix(SYSTEM_SUFFIX)
            .unwrap_or(&platform)
            .to_owned(),
        entity_name,
        registry_name: non_empty(&raw.shop_name),
        default_value: non_empty(&raw.default_value),
        body: path.get(1).cloned(),
        location,
        containers: path,
        starmap_object_guid,
        kind: class.map(|class| class.kind.clone()),
        brand: class.and_then(|class| class.brand.clone()),
        commodity_exchange,
        inventory,
        item_guids,
    })
}
