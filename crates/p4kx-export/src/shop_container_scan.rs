// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Archive, CancelToken, Entry};

use crate::CatalogError;
use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryXml};
use crate::container_read::soc_entries;
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_container_xml;
use crate::shop_containers::{Component, Inventory, verdict};
use crate::shop_entry_read;
use crate::shop_inventory_observations::InventoryObservations;
use crate::shop_soc::{self, Anchors, CapturedScan};
use crate::starmap_index::key;
use crate::starmap_xml::Node;

#[path = "shop_container_entry_read.rs"]
mod entry_read;
use entry_read::{projected, read, record_read_error, rejected};

#[cfg(test)]
#[path = "shop_soc_subtypes.rs"]
pub(crate) mod soc_subtypes;

const SHOP_METADATA: &str = "\\metadata\\entitycomponentshop.xml";
const EDITOR_SUFFIX: &str = "_editor.xml";
const MAX_METADATA_LEN: u64 = 1 << 20;
const MAX_EDITOR_LEN: u64 = 256 << 20;

pub(crate) type Shops = Vec<(String, Component)>;

type GuidReceipts = HashMap<String, Vec<AuthoredEntryPath>>;

type ObjectNames = HashMap<String, (String, Vec<AuthoredEntryPath>)>;

/// The container projection and the actual Entry operations that produced it.
#[derive(Default)]
pub(crate) struct Scanned {
    pub(crate) entities: Option<(Shops, usize)>,
    pub(crate) class_complete: bool,
    pub(crate) guid_receipts: GuidReceipts,
    pub(crate) observations: HashMap<String, InventoryObservations>,
    pub(crate) unattributed_declarations: bool,
    pub(crate) ledger: ExportLedger,
    #[cfg(test)]
    pub(crate) soc_findings: Vec<soc_subtypes::EntryFindings>,
}

#[derive(Default)]
struct PreparedShops {
    shops: Shops,
    class_complete: bool,
    guid_receipts: GuidReceipts,
    #[cfg(test)]
    soc_findings: Vec<soc_subtypes::EntryFindings>,
    rejected: usize,
    observations: HashMap<String, InventoryObservations>,
    unattributed_declarations: bool,
}

pub(crate) fn shop_entities(
    archive: &Archive,
    socpak: &Entry,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Scanned, CatalogError> {
    let mut ledger = ExportLedger::default();
    let Some(prepared) = scan(archive, socpak, &mut ledger, cancel, capture)? else {
        return Ok(Scanned {
            ledger,
            ..Scanned::default()
        });
    };
    Ok(Scanned {
        entities: Some((prepared.shops, prepared.rejected)),
        class_complete: prepared.class_complete,
        guid_receipts: prepared.guid_receipts,
        #[cfg(test)]
        soc_findings: prepared.soc_findings,
        observations: prepared.observations,
        unattributed_declarations: prepared.unattributed_declarations,
        ledger,
    })
}

fn scan(
    archive: &Archive,
    socpak: &Entry,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Option<PreparedShops>, CatalogError> {
    let source = EntrySource::outer(capture, archive, socpak)?.dependency();
    source.receipt().attempted_payload()?;
    let Ok(container) = archive.open_container(socpak) else {
        ledger.incomplete_inventory(crate::Kind::Shops);
        source.record(ledger, crate::Kind::Shops, SourceDisposition::ReadFailed);
        return Ok(None);
    };
    source.record(ledger, crate::Kind::Shops, SourceDisposition::Projected);
    let mut matches = container.entries().iter().filter(|inner| {
        let name = key(inner.path());
        name.ends_with(SHOP_METADATA) || name == SHOP_METADATA.trim_start_matches('\\')
    });
    let directory_complete = crate::export_archive_health::inventory(&container).is_none();
    let Some(metadata) = matches.next() else {
        return Ok(Some(PreparedShops {
            class_complete: directory_complete,
            ..PreparedShops::default()
        }));
    };
    let source = EntrySource::inner(capture, archive, socpak, &container, metadata)?
        .operation(EntryOperation::ShopMetadata)
        .dependency();
    let Some(bytes) = read(
        &container,
        metadata,
        MAX_METADATA_LEN,
        &source,
        ledger,
        capture,
    )?
    else {
        return Ok(None);
    };
    let Ok(xml) = EntryXml::parse(&bytes, capture)? else {
        ledger.incomplete_inventory(crate::Kind::Shops);
        rejected(source, ledger, "Entity.CryGUID");
        return Ok(None);
    };
    let entities = shop_container_xml::captured_guids(&xml)?;
    projected(source, ledger, entities.malformed, "Entity.CryGUID");
    let root = xml.root();
    let name = root.name();
    let complete = directory_complete
        && matches.next().is_none()
        && !entities.malformed
        && !xml.is_lossy()
        && (name.eq_ignore_ascii_case("Entities")
            || name.eq_ignore_ascii_case("EntityComponentShop"));
    let mut result = finish(
        archive,
        socpak,
        &container,
        entities.guids,
        ledger,
        cancel,
        capture,
    )?;
    result.class_complete = complete;
    Ok(Some(result))
}

fn finish(
    archive: &Archive,
    socpak: &Entry,
    container: &Archive,
    guids: Vec<(String, AuthoredEntryPath)>,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<PreparedShops, CatalogError> {
    // Output order includes repeated declarations; the anchor index consumes their receipts.
    let mut entity_ids = Vec::with_capacity(guids.len());
    entity_ids.extend(guids.iter().map(|(guid, _)| guid.clone()));
    let anchors = Anchors::captured(
        guids,
        object_names(archive, container, socpak, ledger, capture)?.unwrap_or_default(),
    );
    let mut components = scan_components(
        archive, container, socpak, &anchors, ledger, cancel, capture,
    )?;
    let mut shops = Vec::with_capacity(entity_ids.len());
    for guid in entity_ids {
        let state = components.resolve(&guid, &anchors)?;
        shops.push((guid, state));
    }
    Ok(PreparedShops {
        shops,
        guid_receipts: anchors.into_guid_receipts(),
        #[cfg(test)]
        soc_findings: components.soc_findings,
        rejected: components.rejected,
        observations: components.observations,
        unattributed_declarations: components.unattributed_declarations,
        ..PreparedShops::default()
    })
}

#[derive(Default)]
struct Components {
    #[cfg(test)]
    soc_findings: Vec<soc_subtypes::EntryFindings>,
    found: HashMap<String, Vec<Component>>,
    observations: HashMap<String, InventoryObservations>,
    unattributed_declarations: bool,
    rejected: usize,
}

impl Components {
    fn retain(
        &mut self,
        observed: CapturedScan,
        source: EntrySource,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        if observed.source_incomplete {
            ledger.incomplete_inventory(crate::Kind::Shops);
        }
        let scan = observed.scan;
        self.unattributed_declarations |= observed.unattributed_declarations || scan.rejected > 0;
        for (entity, rejected) in observed.rejected {
            self.observations
                .entry(entity)
                .or_default()
                .merge(InventoryObservations {
                    rejected,
                    ..InventoryObservations::default()
                })?;
        }
        for (entity, no_file) in observed.no_file {
            self.observations
                .entry(entity)
                .or_default()
                .merge(InventoryObservations {
                    no_file,
                    ..InventoryObservations::default()
                })?;
        }
        projected(
            source,
            ledger,
            scan.rejected > 0 || scan.malformed,
            "offlineInventory",
        );
        self.rejected += scan.rejected;
        for component in scan.components {
            self.found.entry(component.entity).or_default().push(
                component
                    .inventory
                    .map_or(Component::NoFile, Component::File),
            );
        }
        Ok(())
    }

    fn resolve(&mut self, guid: &str, anchors: &Anchors) -> Result<Component, CatalogError> {
        let states = self.found.get(guid).map_or(&[][..], Vec::as_slice);
        let result = if states.is_empty() {
            Component::NotFound
        } else {
            combined(states)?
        };
        match &result {
            Component::Conflict => {
                let original = self.observations.entry(guid.to_owned()).or_default();
                for state in states {
                    if let Component::File(stem) = state {
                        original.conflict(stem.declarations())?;
                    }
                }
            }
            Component::NotFound => self
                .observations
                .entry(guid.to_owned())
                .or_default()
                .missing_component(anchors.guid_receipts(guid))?,
            Component::NoFile | Component::File(_) => {}
        }
        Ok(result)
    }
}

fn scan_components(
    archive: &Archive,
    container: &Archive,
    socpak: &Entry,
    anchors: &Anchors,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<Components, CatalogError> {
    let mut components = Components::default();
    for inner in soc_entries(container) {
        let source = EntrySource::inner(capture, archive, socpak, container, inner)?
            .operation(EntryOperation::ShopSoc)
            .dependency();
        let bytes = match shop_entry_read::read_soc(capture, &source, container, inner, cancel)? {
            Ok(bytes) => bytes,
            Err(error) => {
                record_read_error(source, ledger, &error);
                continue;
            }
        };
        let observed = shop_soc::read_components(&bytes, capture, anchors, archive, cancel)?;
        #[cfg(test)]
        {
            let raw = shop_soc::read_legacy_components(&bytes, capture, anchors)?;
            if let Some(mut findings) = soc_subtypes::findings(&source, &raw)? {
                let probe = soc_subtypes::Probe { capture, anchors };
                findings.inspect(&probe, &bytes, &raw);
                components.soc_findings.push(findings);
            }
        }
        components.retain(observed, source, ledger)?;
    }
    Ok(components)
}

fn combined(states: &[Component]) -> Result<Component, CatalogError> {
    match verdict(states) {
        Inventory::None => Ok(Component::NoFile),
        Inventory::Ambiguous => Ok(Component::Conflict),
        Inventory::Unknown => Ok(Component::NotFound),
        Inventory::One(value) => combine_files(states, value),
    }
}

fn combine_files(states: &[Component], value: &str) -> Result<Component, CatalogError> {
    let mut selected = states.iter().filter_map(|state| match state {
        Component::File(stem) if stem.as_str() == value => Some(stem),
        _ => None,
    });
    let Some(first) = selected.next() else {
        return Err(crate::ExportAccountingError::ConflictingSourceDatum.into());
    };
    let mut stem = first.clone();
    for other in selected {
        stem.merge(other)?;
    }
    Ok(Component::File(stem))
}

fn object_names(
    archive: &Archive,
    container: &Archive,
    socpak: &Entry,
    ledger: &mut ExportLedger,
    capture: &EntryCapture<'_>,
) -> Result<Option<ObjectNames>, CatalogError> {
    let Some(editor) = container.entries().iter().find(|inner| {
        let path = inner.path();
        !path.contains(['\\', '/']) && key(path).ends_with(EDITOR_SUFFIX)
    }) else {
        return Ok(None);
    };
    let source = EntrySource::inner(capture, archive, socpak, container, editor)?
        .operation(EntryOperation::ShopEditor)
        .dependency();
    let Some(bytes) = read(container, editor, MAX_EDITOR_LEN, &source, ledger, capture)? else {
        return Ok(None);
    };
    let Ok(xml) = EntryXml::parse(&bytes, capture)? else {
        ledger.incomplete_inventory(crate::Kind::Shops);
        rejected(source, ledger, "SCShop.name/guid");
        return Ok(None);
    };
    let objects = shop_container_xml::captured_objects(&xml)?;
    projected(source, ledger, objects.malformed, "SCShop.name/guid");
    Ok(Some(objects.names))
}

#[cfg(test)]
#[path = "shop_container_scan_tests.rs"]
mod tests;
