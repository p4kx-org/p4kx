// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::Path;

use p4kx_extraction::{Archive, CancelToken, Entry};
use rayon::prelude::*;

use crate::Kind;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_entry_read;
use crate::shop_soc::inventory_stem;
use crate::starmap_index::key;
use crate::starmap_xml::{Document, Node, attribute};

const FOLDER: &str = "data\\prefabs\\shops\\";
const MAX_PREFAB_LEN: u64 = 64 << 20;
const PROPERTY: &str = "offlineInventory";

/// The inventory files that the shop prefabs of an Archive name, by the `CryGUID` of the Entity.
#[derive(Debug, Default)]
pub(crate) struct Prefabs {
    pub(crate) ledger: ExportLedger,
    pub(crate) by_entity: HashMap<String, Vec<String>>,
    pub(crate) listed: usize,
    pub(crate) unreadable: usize,
}

impl Prefabs {
    /// Reads every prefab library below `Data\Prefabs\Shops`. A shop Entity of a prefab has an
    /// `Id` that is its `CryGUID` with the bytes reordered, which the containers that instance
    /// the prefab carry in their `entitycomponentshop.xml`.
    pub(crate) fn read(archive: &Archive, cancel: Option<&CancelToken>) -> Self {
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
        let read: Vec<ReadPrefab> = listed
            .par_iter()
            .map(|entry| {
                if cancel.is_some_and(CancelToken::is_cancelled) {
                    return ReadPrefab::default();
                }
                named(archive, entry)
            })
            .collect();
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
                let stems = prefabs.by_entity.entry(entity).or_default();
                if !stems.contains(&stem) {
                    stems.push(stem);
                }
            }
        }
        prefabs
    }
}

#[derive(Default)]
struct ReadPrefab {
    pairs: Option<Vec<(String, String)>>,
    ledger: ExportLedger,
}

fn named(archive: &Archive, entry: &Entry) -> ReadPrefab {
    let mut read = ReadPrefab::default();
    let source = EntrySource::outer(entry)
        .operation(EntryOperation::ShopPrefab)
        .dependency();
    let bytes = match shop_entry_read::read(archive, entry, MAX_PREFAB_LEN) {
        Ok(bytes) => bytes,
        Err(error) => {
            read.ledger.incomplete_inventory(Kind::Shops);
            source.diagnosed(
                &mut read.ledger,
                Kind::Shops,
                SourceDisposition::ReadFailed,
                error.diagnosis(),
            );
            return read;
        }
    };
    let Ok(document) = Document::parse(&bytes) else {
        read.ledger.incomplete_inventory(Kind::Shops);
        source.diagnosed(
            &mut read.ledger,
            Kind::Shops,
            SourceDisposition::ProjectionFailed,
            shop_entry_read::diagnosis(ExportDiagnosticCode::MalformedSource, "PrefabLibrary"),
        );
        return read;
    };
    let projected = pairs(document.root());
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
    read.pairs = Some(projected.pairs);
    read
}

#[derive(Default)]
struct Pairs {
    pairs: Vec<(String, String)>,
    failure: Option<&'static str>,
}

fn pairs<'d, N: Node<'d>>(root: N) -> Pairs {
    let mut found = Pairs::default();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.name().eq_ignore_ascii_case("Object") && attribute(node, "Type") == Some("Entity") {
            let stems = shop_stems(node);
            if stems.malformed {
                found
                    .failure
                    .get_or_insert("EntityComponentShop.offlineInventory");
            }
            if !stems.files.is_empty() {
                if let Some(entity) = attribute(node, "Id").and_then(crypt_guid) {
                    found
                        .pairs
                        .extend(stems.files.into_iter().map(|stem| (entity.clone(), stem)));
                } else {
                    found.failure.get_or_insert("Object.Id");
                }
            }
        }
        pending.extend(node.children());
    }
    found
}

#[derive(Default)]
struct Stems {
    files: Vec<String>,
    malformed: bool,
}

fn shop_stems<'d, N: Node<'d>>(object: N) -> Stems {
    let mut found = Stems::default();
    for (_, value) in object
        .children()
        .filter(|child| child.name().eq_ignore_ascii_case("PropertiesDataCore"))
        .flat_map(N::children)
        .filter(|child| child.name().eq_ignore_ascii_case("EntityComponentShop"))
        .flat_map(|component| component.attributes().collect::<Vec<_>>())
        .filter(|(name, _)| {
            name.get(..PROPERTY.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(PROPERTY))
        })
    {
        if let Some(stem) = inventory_stem(value) {
            found.files.push(stem);
        } else if !value.trim().is_empty() {
            found.malformed = true;
        }
    }
    found
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
