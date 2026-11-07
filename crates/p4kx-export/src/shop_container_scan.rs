// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{Archive, Entry};

use crate::Kind;
use crate::container_read::{MAX_SOC_LEN, soc_entries};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_containers::{Component, Inventory, verdict};
use crate::shop_entry_read;
use crate::shop_registry::is_guid;
use crate::shop_soc::{self, Anchors};
use crate::starmap_index::key;
use crate::starmap_xml::{Document, Node, attribute};

const SHOP_METADATA: &str = "\\metadata\\entitycomponentshop.xml";
const EDITOR_SUFFIX: &str = "_editor.xml";
const SHOP_TYPE: &str = "SCShop";
const MAX_METADATA_LEN: u64 = 1 << 20;
const MAX_EDITOR_LEN: u64 = 256 << 20;

/// The `CryGUID`s of a container with what its `.soc` files say of each one's component.
pub(crate) type Shops = Vec<(String, Component)>;

/// A container's shop projection and the actual Entry operations that produced it.
#[derive(Default)]
pub(crate) struct Scanned {
    pub(crate) entities: Option<(Shops, usize)>,
    pub(crate) ledger: ExportLedger,
}

/// Returns the `CryGUID`s `entitycomponentshop.xml` of the container `socpak` names, lower case,
/// each with what the `.soc` files of the container say of its component; `None` when the
/// container or its `entitycomponentshop.xml` cannot be read, and how many inventory paths of its
/// `.soc` files no Entity could be told for. A `.soc` that cannot be read only
/// leaves the component `NotFound`: kind and brand do not depend on it.
pub(crate) fn shop_entities(archive: &Archive, socpak: &Entry) -> Scanned {
    let mut ledger = ExportLedger::default();
    let entities = scan(archive, socpak, &mut ledger);
    Scanned { entities, ledger }
}

fn scan(archive: &Archive, socpak: &Entry, ledger: &mut ExportLedger) -> Option<(Shops, usize)> {
    let source = EntrySource::outer(socpak).dependency();
    let Ok(container) = archive.open_container(socpak) else {
        ledger.incomplete_inventory(Kind::Shops);
        source.record(ledger, Kind::Shops, SourceDisposition::ReadFailed);
        return None;
    };
    source.record(ledger, Kind::Shops, SourceDisposition::Projected);
    let Some(metadata) = container.entries().iter().find(|inner| {
        let name = key(inner.path());
        name.ends_with(SHOP_METADATA) || name == SHOP_METADATA.trim_start_matches('\\')
    }) else {
        return Some((Vec::new(), 0));
    };
    let source = EntrySource::inner(socpak, metadata)
        .operation(EntryOperation::ShopMetadata)
        .dependency();
    let bytes = read(&container, metadata, MAX_METADATA_LEN, &source, ledger)?;
    let Some(entities) = entities(&bytes) else {
        ledger.incomplete_inventory(Kind::Shops);
        rejected(source, ledger, "Entity.CryGUID");
        return None;
    };
    projected(source, ledger, entities.malformed, "Entity.CryGUID");
    let guids = entities.guids;
    let anchors = Anchors::new(
        &guids,
        object_names(&container, socpak, ledger).unwrap_or_default(),
    );
    let components = scan_components(&container, socpak, &anchors, ledger);
    let shops = guids
        .into_iter()
        .map(|guid| {
            let state = components
                .found
                .get(&guid)
                .map_or(Component::NotFound, |states| combined(states));
            (guid, state)
        })
        .collect();
    Some((shops, components.rejected))
}

#[derive(Default)]
struct Components {
    found: HashMap<String, Vec<Component>>,
    rejected: usize,
}

fn scan_components(
    container: &Archive,
    socpak: &Entry,
    anchors: &Anchors,
    ledger: &mut ExportLedger,
) -> Components {
    let mut components = Components::default();
    for inner in soc_entries(container) {
        let source = EntrySource::inner(socpak, inner)
            .operation(EntryOperation::ShopSoc)
            .dependency();
        let Some(bytes) = read(container, inner, MAX_SOC_LEN, &source, ledger) else {
            continue;
        };
        let scan = shop_soc::components(&bytes, anchors);
        projected(
            source,
            ledger,
            scan.rejected > 0 || scan.malformed,
            "offlineInventory",
        );
        components.rejected += scan.rejected;
        for component in scan.components {
            components.found.entry(component.entity).or_default().push(
                component
                    .inventory
                    .map_or(Component::NoFile, Component::File),
            );
        }
    }
    components
}

fn combined(states: &[Component]) -> Component {
    match verdict(states) {
        Inventory::None => Component::NoFile,
        Inventory::One(stem) => Component::File(stem.to_owned()),
        Inventory::Ambiguous => Component::Conflict,
        Inventory::Unknown => Component::NotFound,
    }
}

fn entities(bytes: &[u8]) -> Option<Entities> {
    Some(guids(Document::parse(bytes).ok()?.root()))
}

struct Entities {
    guids: Vec<String>,
    malformed: bool,
}

fn guids<'d, N: Node<'d>>(root: N) -> Entities {
    let mut found = Entities {
        guids: Vec::new(),
        malformed: false,
    };
    for entity in root
        .children()
        .filter(|child| child.name().eq_ignore_ascii_case("Entity"))
    {
        if let Some(guid) = attribute(entity, "CryGUID") {
            found.malformed |= !is_guid(guid);
            found.guids.push(guid.to_ascii_lowercase());
        } else {
            found.malformed = true;
        }
    }
    found
}

fn object_names(
    container: &Archive,
    socpak: &Entry,
    ledger: &mut ExportLedger,
) -> Option<HashMap<String, String>> {
    let editor = container.entries().iter().find(|inner| {
        let path = inner.path();
        !path.contains(['\\', '/']) && key(path).ends_with(EDITOR_SUFFIX)
    })?;
    let source = EntrySource::inner(socpak, editor)
        .operation(EntryOperation::ShopEditor)
        .dependency();
    let bytes = read(container, editor, MAX_EDITOR_LEN, &source, ledger)?;
    let Ok(document) = Document::parse(&bytes) else {
        ledger.incomplete_inventory(Kind::Shops);
        rejected(source, ledger, "SCShop.name/guid");
        return None;
    };
    let objects = shop_objects(document.root());
    projected(source, ledger, objects.malformed, "SCShop.name/guid");
    Some(objects.names)
}

struct Objects {
    names: HashMap<String, String>,
    malformed: bool,
}

fn shop_objects<'d, N: Node<'d>>(root: N) -> Objects {
    let mut named: HashMap<&str, Option<String>> = HashMap::new();
    let mut malformed = false;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node.name().eq_ignore_ascii_case("Object") && attribute(node, "type") == Some(SHOP_TYPE)
        {
            let (Some(name), Some(guid)) = (attribute(node, "name"), attribute(node, "guid"))
            else {
                malformed = true;
                pending.extend(node.children());
                continue;
            };
            malformed |= name.is_empty() || !is_guid(guid);
            let guid = guid.to_ascii_lowercase();
            named
                .entry(name)
                .and_modify(|held| {
                    if held.as_ref() != Some(&guid) {
                        malformed = true;
                        *held = None;
                    }
                })
                .or_insert(Some(guid));
        }
        pending.extend(node.children());
    }
    Objects {
        names: named
            .into_iter()
            .filter_map(|(name, guid)| guid.map(|guid| (name.to_owned(), guid)))
            .collect(),
        malformed,
    }
}

fn read(
    container: &Archive,
    entry: &Entry,
    limit: u64,
    source: &EntrySource,
    ledger: &mut ExportLedger,
) -> Option<Vec<u8>> {
    match shop_entry_read::read(container, entry, limit) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            ledger.incomplete_inventory(Kind::Shops);
            source.clone().diagnosed(
                ledger,
                Kind::Shops,
                SourceDisposition::ReadFailed,
                error.diagnosis(),
            );
            None
        }
    }
}

fn rejected(source: EntrySource, ledger: &mut ExportLedger, field: &str) {
    source.diagnosed(
        ledger,
        Kind::Shops,
        SourceDisposition::ProjectionFailed,
        shop_entry_read::diagnosis(ExportDiagnosticCode::MalformedSource, field),
    );
}

fn projected(source: EntrySource, ledger: &mut ExportLedger, malformed: bool, field: &str) {
    if malformed {
        rejected(source, ledger, field);
    } else {
        source.record(ledger, Kind::Shops, SourceDisposition::Projected);
    }
}

#[cfg(test)]
mod tests {
    use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

    use super::*;

    const METADATA: &str =
        r#"<Entities><Entity CryGUID="AB-1"/><Other/><Entity CryGUID="cd-2"/></Entities>"#;

    #[test]
    fn the_shop_entities_are_read_from_text_xml_with_or_without_a_byte_order_mark() {
        let expected = Some(vec!["ab-1".to_owned(), "cd-2".to_owned()]);
        assert_eq!(
            entities(METADATA.as_bytes()).map(|scan| scan.guids),
            expected
        );
        assert_eq!(
            entities(format!("\u{feff}{METADATA}").as_bytes()).map(|scan| scan.guids),
            expected
        );
    }

    #[test]
    fn the_shop_entities_are_read_from_cryxml() {
        let cry = TestElement::new("Entities")
            .child(TestElement::new("Entity").attribute("CryGUID", "AB-1"))
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
        assert_eq!(
            entities(&cry).map(|scan| scan.guids),
            Some(vec!["ab-1".to_owned()])
        );
    }

    #[test]
    fn bytes_that_are_no_xml_give_no_shop_entities() {
        assert!(entities(b"not xml").is_none(), "malformed text is not XML");
        assert!(
            entities(b"<a b=\"\xff\"/>").is_none(),
            "attribute bytes must be UTF-8"
        );
        assert!(
            entities(b"<a><b></a>").is_none(),
            "unclosed XML is not a source"
        );
    }

    #[test]
    fn a_shop_object_name_that_two_guids_claim_is_dropped_in_text_xml_with_a_mark() {
        let editor = "\u{feff}<Root><Object type=\"SCShop\" name=\"one\" guid=\"A\"/>\
            <Object type=\"SCShop\" name=\"two\" guid=\"B\"/>\
            <Object type=\"SCShop\" name=\"two\" guid=\"C\"/>\
            <Object type=\"Other\" name=\"three\" guid=\"D\"/></Root>";
        let document = Document::parse(editor.as_bytes()).expect("text XML");
        let named = shop_objects(document.root()).names;
        assert_eq!(named.get("one").map(String::as_str), Some("a"));
        assert!(!named.contains_key("two"), "ambiguous");
        assert!(!named.contains_key("three"), "not a shop");
    }
}
