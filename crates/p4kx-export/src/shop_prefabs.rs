// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, Entry};
use rayon::prelude::*;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, EntryXml};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_entry_read;
use crate::shop_inventory_name;
use crate::shop_inventory_observations::RejectedInventoryPath;
use crate::shop_inventory_path::InventoryStem;
use crate::starmap_index::key;
use crate::starmap_xml::{Node, attribute};
use crate::{CatalogError, ExportAccountingError, Kind};

const FOLDER: &str = "data\\prefabs\\shops\\";
const MAX_PREFAB_LEN: u64 = 64 << 20;
const PROPERTY: &str = "offlineInventory";

/// Inventory selections from the original shop-prefab XML attributes.
#[derive(Debug, Default)]
pub(crate) struct Prefabs {
    pub(crate) ledger: ExportLedger,
    pub(crate) by_entity: HashMap<String, Vec<InventoryStem>>,
    pub(crate) rejected: HashMap<String, Vec<RejectedInventoryPath>>,
    pub(crate) unattributed_declarations: bool,
    pub(crate) listed: usize,
    pub(crate) unreadable: usize,
}

impl Prefabs {
    pub(crate) fn read(
        archive: &Archive,
        cancel: Option<&CancelToken>,
        capture: &EntryCapture<'_>,
    ) -> Result<Self, CatalogError> {
        let listed: Vec<&Entry> = archive
            .entries()
            .iter()
            .filter(|entry| {
                let path = key(entry.path());
                path.starts_with(FOLDER)
                    && Path::new(&path)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
            })
            .collect();
        let read = listed
            .par_iter()
            .map(|entry| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return Ok(ReadPrefab::default());
                }
                named(archive, entry, capture)
            })
            .collect::<Result<Vec<_>, CatalogError>>()?;
        let mut prefabs = Self {
            listed: listed.len(),
            ..Self::default()
        };
        for read in read {
            prefabs.ledger.merge(read.ledger);
            let Some(pairs) = read.pairs else {
                prefabs.unreadable += 1;
                continue;
            };
            for (entity, stem) in pairs {
                insert(prefabs.by_entity.entry(entity).or_default(), stem)?;
            }
            for (entity, path) in read.rejected {
                prefabs.rejected.entry(entity).or_default().push(path);
            }
            prefabs.unattributed_declarations |= read.unattributed_declarations;
        }
        Ok(prefabs)
    }
}

fn insert(stems: &mut Vec<InventoryStem>, stem: InventoryStem) -> Result<(), CatalogError> {
    if let Some(held) = stems.iter_mut().find(|held| **held == stem) {
        held.merge(&stem)?;
    } else {
        stems.push(stem);
    }
    Ok(())
}

#[derive(Default)]
struct ReadPrefab {
    pairs: Option<Vec<(String, InventoryStem)>>,
    rejected: Vec<(String, RejectedInventoryPath)>,
    unattributed_declarations: bool,
    ledger: ExportLedger,
}

fn named(
    archive: &Archive,
    entry: &Entry,
    capture: &EntryCapture<'_>,
) -> Result<ReadPrefab, CatalogError> {
    let mut read = ReadPrefab::default();
    let source = EntrySource::outer(capture, archive, entry)?
        .operation(EntryOperation::ShopPrefab)
        .dependency();
    let bytes = match shop_entry_read::read(capture, &source, archive, entry, MAX_PREFAB_LEN)? {
        Ok(bytes) => bytes,
        Err(error) => {
            read.ledger.incomplete_inventory(Kind::Shops);
            source.diagnosed(
                &mut read.ledger,
                Kind::Shops,
                SourceDisposition::ReadFailed,
                error.diagnosis(),
            );
            return Ok(read);
        }
    };
    let Ok(xml) = EntryXml::parse(&bytes, capture)? else {
        read.ledger.incomplete_inventory(Kind::Shops);
        source.diagnosed(
            &mut read.ledger,
            Kind::Shops,
            SourceDisposition::ProjectionFailed,
            shop_entry_read::diagnosis(ExportDiagnosticCode::MalformedSource, "PrefabLibrary"),
        );
        return Ok(read);
    };
    let projected = pairs(&xml, capture)?;
    if let Some(field) = projected.failure {
        source.diagnosed(
            &mut read.ledger,
            Kind::Shops,
            SourceDisposition::ProjectionFailed,
            shop_entry_read::diagnosis(ExportDiagnosticCode::MalformedSource, field),
        );
    } else {
        source.record(&mut read.ledger, Kind::Shops, SourceDisposition::Projected);
    }
    read.pairs = Some(projected.files);
    read.rejected = projected.rejected;
    read.unattributed_declarations = projected.unattributed_declarations;
    Ok(read)
}

#[derive(Default)]
struct Pairs {
    files: Vec<(String, InventoryStem)>,
    rejected: Vec<(String, RejectedInventoryPath)>,
    unattributed_declarations: bool,
    failure: Option<&'static str>,
}

fn pairs(xml: &EntryXml<'_, '_, '_>, capture: &EntryCapture<'_>) -> Result<Pairs, CatalogError> {
    let mut found = Pairs::default();
    let mut pending = vec![xml.root()];
    while let Some(node) = pending.pop() {
        if node.name().eq_ignore_ascii_case("Object") && attribute(node, "Type") == Some("Entity") {
            let stems = shop_stems(node, xml)?;
            if !stems.rejected.is_empty() {
                found
                    .failure
                    .get_or_insert("EntityComponentShop.offlineInventory");
            }
            if !stems.files.is_empty() || !stems.rejected.is_empty() {
                append(&mut found, stems, node, xml, capture)?;
            }
        }
        pending.extend(node.children());
    }
    Ok(found)
}

fn append<'node>(
    found: &mut Pairs,
    stems: Stems,
    node: impl Node<'node>,
    xml: &EntryXml<'_, '_, '_>,
    capture: &EntryCapture<'_>,
) -> Result<(), CatalogError> {
    let Some(entity) = attribute(node, "Id").and_then(crypt_guid) else {
        found.failure.get_or_insert("Object.Id");
        found.unattributed_declarations = true;
        return Ok(());
    };
    let id = xml
        .attribute(node, "Id")?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let kind = xml
        .attribute(node, "Type")?
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let name = xml.name(node)?;
    let select = |path: AuthoredEntryPath| -> Result<AuthoredEntryPath, CatalogError> {
        Ok(path
            .selected_by(&id)?
            .selected_by(&kind)?
            .selected_by(&name)?)
    };
    for (path, stem) in stems.files {
        let path = select(path)?;
        found.files.push((
            entity.clone(),
            InventoryStem::captured(capture, &path, stem)?,
        ));
    }
    for path in stems.rejected {
        found
            .rejected
            .push((entity.clone(), RejectedInventoryPath::Text(select(path)?)));
    }
    Ok(())
}

#[derive(Default)]
struct Stems {
    files: Vec<(AuthoredEntryPath, String)>,
    rejected: Vec<AuthoredEntryPath>,
}

fn shop_stems<'node>(
    object: impl Node<'node>,
    xml: &EntryXml<'_, '_, '_>,
) -> Result<Stems, CatalogError> {
    let mut found = Stems::default();
    for properties in object
        .children()
        .filter(|child| child.name().eq_ignore_ascii_case("PropertiesDataCore"))
    {
        for component in properties
            .children()
            .filter(|child| child.name().eq_ignore_ascii_case("EntityComponentShop"))
        {
            for (ordinal, (_, value)) in
                component.attributes().enumerate().filter(|(_, (name, _))| {
                    name.get(..PROPERTY.len())
                        .is_some_and(|head| head.eq_ignore_ascii_case(PROPERTY))
                })
            {
                if value.trim().is_empty() {
                    continue;
                }
                let path = xml
                    .attribute_at(component, ordinal)?
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                    .selected_by(&xml.name(properties)?)?
                    .selected_by(&xml.name(component)?)?;
                if let Some(stem) = shop_inventory_name::stem(value) {
                    found.files.push((path, stem));
                } else {
                    found.rejected.push(path);
                }
            }
        }
    }
    Ok(found)
}

/// Returns the lower-case `CryGUID` of the editor `Id` `{AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE}`:
/// `CCCCBBBB-AAAA(first four)-AAAA(last four)-` and the bytes `DDDDEEEEEEEEEEEE` reversed.
fn crypt_guid(id: &str) -> Option<String> {
    let hex: String = id
        .trim_matches(['{', '}'])
        .chars()
        .filter(|c| *c != '-')
        .collect::<String>()
        .to_ascii_lowercase();
    if hex.len() != 32 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let (a, rest) = hex.split_at(8);
    let (b, rest) = rest.split_at(4);
    let (c, tail) = rest.split_at(4);
    let reversed: String = (0..tail.len() / 2)
        .rev()
        .map(|at| &tail[at * 2..at * 2 + 2])
        .collect();
    Some(format!(
        "{c}{b}-{}-{}-{}-{}",
        &a[..4],
        &a[4..],
        &reversed[..4],
        &reversed[4..]
    ))
}

#[cfg(test)]
mod tests {
    use super::crypt_guid;

    #[test]
    fn the_editor_id_becomes_the_container_guid() {
        assert_eq!(
            crypt_guid("{9730AC77-4077-4DCE-B700-273B6B917450}").as_deref(),
            Some("4dce4077-9730-ac77-5074-916b3b2700b7")
        );
        assert_eq!(crypt_guid("{9730AC77-4077}"), None);
        assert_eq!(crypt_guid("{ZZ30AC77-4077-4DCE-B700-273B6B917450}"), None);
    }
}
