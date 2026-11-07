// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeSet, HashMap};

use p4kx_extraction::{Archive, CancelToken, Entry};
use rayon::prelude::*;

use crate::CatalogError;
use crate::export_ledger::ExportLedger;
use crate::shop_container_scan::{Scanned, shop_entities};
use crate::shop_prefabs::Prefabs;
use crate::starmap_index::key;

const SHOPS: &str = "data\\objectcontainers\\pu\\shops\\";
const LOC: &str = "data\\objectcontainers\\pu\\loc\\";
const LOC_KINDS: [&str; 2] = ["shopcomm", "shopfood"];
const SCANNED: [&str; 2] = [
    "data\\objectcontainers\\pu\\",
    "data\\objectcontainers\\ea\\",
];

/// The kind and the brand of a shop container, from its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Class {
    pub(crate) kind: String,
    pub(crate) brand: Option<String>,
}

/// What the containers say of one `SCShop` Entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found<'a> {
    /// No shop-folder container names the Entity; kind and brand are unknown.
    Nowhere,
    /// Every container that names it has this kind and brand.
    One(&'a Class),
    /// The containers that name it differ in kind or brand.
    Mixed,
}

/// What the containers say of the inventory file of one `SCShop` Entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Inventory<'a> {
    /// No container names the Entity, or every container that does has the component and it
    /// names no inventory file.
    None,
    /// Every container that names the Entity names this file (lower-case stem).
    One(&'a str),
    /// The containers differ, or only some of them name a file.
    Ambiguous,
    /// No container that names the Entity has its component in a readable `.soc`, so nothing is
    /// known.
    Unknown,
}

/// What one container's `.soc` files say of the component of one `SCShop` Entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Component {
    /// No readable `.soc` of the container holds the component.
    NotFound,
    /// The component is there and names no inventory file.
    NoFile,
    /// The component names this file (lower-case stem).
    File(String),
    /// The container's components for the Entity disagree.
    Conflict,
}

#[derive(Debug, Default)]
struct Named {
    classes: Vec<Class>,
    inventories: Vec<Component>,
    placed: bool,
    prefab: bool,
}

/// Combines what several containers or `.soc` files say of one Entity.
pub(crate) fn verdict(states: &[Component]) -> Inventory<'_> {
    let mut file: Option<&str> = None;
    let (mut conflict, mut not_found, mut no_file) = (false, false, false);
    for state in states {
        match state {
            Component::File(stem) => match file {
                Some(held) if held != stem => conflict = true,
                _ => file = Some(stem),
            },
            Component::Conflict => conflict = true,
            Component::NotFound => not_found = true,
            Component::NoFile => no_file = true,
        }
    }
    match file {
        _ if conflict => Inventory::Ambiguous,
        Some(_) if not_found || no_file => Inventory::Ambiguous,
        Some(stem) => Inventory::One(stem),
        None if not_found || states.is_empty() => Inventory::Unknown,
        None => Inventory::None,
    }
}

/// The `SCShop` Entities of every container below `PU` and `EA`, with the kind and brand of the
/// shop-folder containers among them and the inventory file their components and the shop prefabs
/// name.
#[derive(Debug, Default)]
pub(crate) struct Containers {
    pub(crate) ledger: ExportLedger,
    by_entity: HashMap<String, Named>,
    pub(crate) listed: usize,
    pub(crate) unreadable: usize,
    pub(crate) with_shop: usize,
    pub(crate) components: usize,
    pub(crate) rejected: usize,
    pub(crate) other_scanned: usize,
    pub(crate) other_unreadable: usize,
    pub(crate) other_with_shop: usize,
}

impl Containers {
    /// Opens every socpak below `PU\Shops` and the `shopcomm` and `shopfood` folders below
    /// `PU\loc`, which give kind and brand, and every other socpak below `PU` and `EA`, and reads
    /// the `CryGUID`s of its `entitycomponentshop.xml` and the shop components of its `.soc`.
    pub(crate) fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let listed = listed(archive);
        let read: Vec<Scanned> = listed
            .par_iter()
            .map(|(entry, _)| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Scanned::default();
                }
                shop_entities(archive, entry)
            })
            .collect();
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let mut containers = Self::default();
        for ((_, class), entities) in listed.into_iter().zip(read) {
            if class.is_some() {
                containers.listed += 1;
            } else {
                containers.other_scanned += 1;
            }
            containers.ledger.merge(entities.ledger);
            let Some((entities, rejected)) = entities.entities else {
                if class.is_some() {
                    containers.unreadable += 1;
                } else {
                    containers.other_unreadable += 1;
                }
                continue;
            };
            containers.rejected += rejected;
            if !entities.is_empty() {
                if class.is_some() {
                    containers.with_shop += 1;
                } else {
                    containers.other_with_shop += 1;
                }
            }
            for (entity, inventory) in entities {
                containers.components += usize::from(inventory != Component::NotFound);
                let named = containers.by_entity.entry(entity).or_default();
                named.placed = true;
                if let Some(class) = &class
                    && !named.classes.contains(class)
                {
                    named.classes.push(class.clone());
                }
                if !named.inventories.contains(&inventory) {
                    named.inventories.push(inventory);
                }
            }
        }
        Ok(containers)
    }

    /// Adds the inventory files the shop prefabs name as one more place per Entity.
    pub(crate) fn add_prefabs(&mut self, prefabs: &Prefabs) {
        for (entity, stems) in &prefabs.by_entity {
            let named = self.by_entity.entry(entity.clone()).or_default();
            named.prefab = true;
            for stem in stems {
                let state = Component::File(stem.clone());
                if !named.inventories.contains(&state) {
                    named.inventories.push(state);
                }
            }
        }
    }

    /// Returns whether a prefab names the Entity and no container holds it.
    pub(crate) fn only_prefab(&self, entity: &str) -> bool {
        self.by_entity
            .get(&entity.to_ascii_lowercase())
            .is_some_and(|named| named.prefab && !named.placed)
    }

    /// Returns what the containers say of the Entity with the `CryGUID` `entity`, ignoring case.
    pub(crate) fn find(&self, entity: &str) -> Found<'_> {
        match self
            .by_entity
            .get(&entity.to_ascii_lowercase())
            .map(|named| named.classes.as_slice())
        {
            None | Some([]) => Found::Nowhere,
            Some([class]) => Found::One(class),
            Some(_) => Found::Mixed,
        }
    }

    /// Returns the inventory file the containers name for the Entity with the `CryGUID` `entity`,
    /// ignoring case.
    pub(crate) fn inventory(&self, entity: &str) -> Inventory<'_> {
        self.by_entity
            .get(&entity.to_ascii_lowercase())
            .map_or(Inventory::None, |named| verdict(&named.inventories))
    }

    /// Returns the inventory file stems any container names, ordered.
    pub(crate) fn inventory_stems(&self) -> BTreeSet<&str> {
        self.by_entity
            .values()
            .flat_map(|named| &named.inventories)
            .filter_map(|state| match state {
                Component::File(stem) => Some(stem.as_str()),
                _ => None,
            })
            .collect()
    }
}

fn listed(archive: &Archive) -> Vec<(&Entry, Option<Class>)> {
    archive
        .entries()
        .iter()
        .filter_map(|entry| {
            let path = key(entry.path());
            let class = classify(&path);
            (class.is_some()
                || (path.ends_with(".socpak")
                    && SCANNED.iter().any(|folder| path.starts_with(folder))))
            .then_some((entry, class))
        })
        .collect()
}

/// Returns the kind and brand of the socpak with the normalised path `path`, `None` when it is no
/// shop container.
pub(crate) fn classify(path: &str) -> Option<Class> {
    if !path.ends_with(".socpak") {
        return None;
    }
    if let Some(rest) = path.strip_prefix(SHOPS) {
        let mut parts = rest.split('\\');
        let kind = parts
            .next()
            .filter(|kind| !kind.is_empty() && rest.contains('\\'))?;
        let brand = parts
            .next()
            .filter(|brand| !brand.is_empty() && parts.next().is_some());
        return Some(Class {
            kind: kind.to_owned(),
            brand: brand.map(str::to_owned),
        });
    }
    let rest = path.strip_prefix(LOC)?;
    let mut folders: Vec<&str> = rest.split('\\').collect();
    folders.pop();
    let kind = folders.last().copied()?;
    LOC_KINDS.contains(&kind).then(|| Class {
        kind: kind.to_owned(),
        brand: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{Component, Inventory, classify, verdict};

    fn file(stem: &str) -> Component {
        Component::File(stem.to_owned())
    }

    #[test]
    fn containers_agree_only_when_all_name_the_same_file() {
        use Component::{Conflict, NoFile, NotFound};
        assert_eq!(verdict(&[]), Inventory::Unknown);
        assert_eq!(verdict(&[file("a")]), Inventory::One("a"));
        assert_eq!(verdict(&[file("a"), file("a")]), Inventory::One("a"));
        assert_eq!(verdict(&[file("a"), file("b")]), Inventory::Ambiguous);
        assert_eq!(verdict(&[NoFile, file("a")]), Inventory::Ambiguous);
        assert_eq!(verdict(&[file("a"), NoFile]), Inventory::Ambiguous);
        assert_eq!(verdict(&[file("a"), NotFound]), Inventory::Ambiguous);
        assert_eq!(verdict(&[Conflict]), Inventory::Ambiguous);
        assert_eq!(verdict(&[NoFile]), Inventory::None);
        assert_eq!(verdict(&[NoFile, NotFound]), Inventory::Unknown);
        assert_eq!(verdict(&[NotFound]), Inventory::Unknown);
    }

    #[test]
    fn kind_and_brand_come_from_the_folders() {
        let class = classify("data\\objectcontainers\\pu\\shops\\commex\\tdd\\tdd_newbab.socpak");
        assert_eq!(class.as_ref().map(|c| c.kind.as_str()), Some("commex"));
        assert_eq!(
            class.and_then(|c| c.brand),
            Some("tdd".to_owned()),
            "the second folder"
        );
        let bare = classify("data\\objectcontainers\\pu\\shops\\admin\\admin_portolisar.socpak");
        assert_eq!(bare.as_ref().map(|c| c.kind.as_str()), Some("admin"));
        assert_eq!(
            bare.and_then(|c| c.brand),
            None,
            "a file straight in the kind folder"
        );
        let module = classify(
            "data\\objectcontainers\\pu\\loc\\mod\\station\\util_a\\int\\module\\shopfood\\x.socpak",
        );
        assert_eq!(module.as_ref().map(|c| c.kind.as_str()), Some("shopfood"));
        assert_eq!(module.and_then(|c| c.brand), None);
    }

    #[test]
    fn other_containers_are_no_shop_containers() {
        for path in [
            "data\\objectcontainers\\pu\\loc\\mod\\station\\util_a\\int\\module\\other\\x.socpak",
            "data\\objectcontainers\\pu\\shops\\admin\\readme.txt",
            "data\\objectcontainers\\pu\\shops\\stray.socpak",
            "data\\objectcontainers\\ships\\x.socpak",
        ] {
            assert_eq!(classify(path), None, "{path}");
        }
    }
}
