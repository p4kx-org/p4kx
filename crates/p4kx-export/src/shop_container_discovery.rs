// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry, SourceUse};
use rayon::prelude::*;

use super::{Class, Component, Containers, append_inventory};
use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, EntryReceipt};
use crate::shop_container_scan::{Scanned, shop_entities};
use crate::starmap_index::key;

const SHOPS: &str = "data\\objectcontainers\\pu\\shops\\";
const LOC: &str = "data\\objectcontainers\\pu\\loc\\";
const LOC_KINDS: [&str; 2] = ["shopcomm", "shopfood"];
const SCANNED: [&str; 2] = [
    "data\\objectcontainers\\pu\\",
    "data\\objectcontainers\\ea\\",
];

impl Containers {
    /// Opens every socpak below `PU\Shops` and the `shopcomm` and `shopfood` folders below
    /// `PU\loc`, which give kind and brand, and every other socpak below `PU` and `EA`, and reads
    /// the `CryGUID`s of its `entitycomponentshop.xml` and the shop components of its `.soc`.
    pub(crate) fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        capture: &EntryCapture<'_>,
    ) -> Result<Self, CatalogError> {
        let listed = listed(archive);
        let class_directory = capture
            .source_uses()
            .observe_root_directory_paths(archive)?;
        let mut containers = Self {
            class_complete: class_directory.is_some()
                && crate::export_archive_health::inventory(archive).is_none(),
            class_directory,
            ..Self::default()
        };
        for window in listed.chunks(rayon::current_num_threads()) {
            let scanned = window
                .par_iter()
                .map(|(entry, _)| {
                    if cancel.is_some_and(CancelToken::is_cancelled) {
                        return Err(CatalogError::Cancelled);
                    }
                    shop_entities(archive, entry, cancel, capture)
                })
                .collect::<Result<Vec<_>, CatalogError>>()?;
            for ((entry, class), entities) in window.iter().zip(scanned) {
                let classification = class
                    .as_ref()
                    .map(|class| {
                        let path = EntryReceipt::outer(capture, archive, entry)?;
                        Ok::<_, CatalogError>((class, path.directory_path_selector()?))
                    })
                    .transpose()?;
                containers.retain(classification.as_ref(), entities)?;
            }
        }
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        Ok(containers)
    }

    fn retain(
        &mut self,
        classification: Option<&(&Class, SourceUse)>,
        mut scanned: Scanned,
    ) -> Result<(), CatalogError> {
        if classification.is_some() {
            self.class_complete &= scanned.class_complete;
            self.listed += 1;
        } else {
            self.other_scanned += 1;
        }
        self.ledger.merge(scanned.ledger);
        #[cfg(test)]
        self.soc_findings.extend(scanned.soc_findings);
        self.unattributed_declarations |= scanned.unattributed_declarations;
        let mut observations = scanned.observations;
        let Some((entities, rejected)) = scanned.entities else {
            if classification.is_some() {
                self.unreadable += 1;
            } else {
                self.other_unreadable += 1;
            }
            return Ok(());
        };
        self.rejected += rejected;
        if !entities.is_empty() {
            if classification.is_some() {
                self.with_shop += 1;
            } else {
                self.other_with_shop += 1;
            }
        }
        for (entity, inventory) in entities {
            self.components += usize::from(inventory != Component::NotFound);
            let original = observations.remove(&entity);
            let guid_receipts = scanned.guid_receipts.remove(&entity);
            let named = self.by_entity.entry(entity).or_default();
            if let Some(original) = original {
                named.observations.merge(original)?;
            }
            named.placed = true;
            if let (Some((class, path)), Some(guids)) = (classification, guid_receipts) {
                named.classes.observe(class, path, &guids)?;
            }
            append_inventory(&mut named.inventories, inventory)?;
        }
        Ok(())
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
pub(super) fn classify(path: &str) -> Option<Class> {
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
