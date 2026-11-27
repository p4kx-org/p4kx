// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read as _;

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, Entry, Guid, PhysicalSourceRef, SourceUseDescription,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::catalog_read::original::with_entry_capture;
use crate::starmap_containers::{PlacementScope, Placements};
use crate::starmap_xml::{Document, Node, Root, attribute};

use super::{TestResult, assert_attribute, original_descriptor_ref, source};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_location_entxml_preserves_authored_harvestable_occurrences() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let Some(witness) = source::find_witness(&archive, &watch)? else {
        return require_entxml();
    };
    let body = archive.open_container(witness.body)?;
    let descriptor = body
        .entries()
        .get(witness.body_member)
        .ok_or("original body XML")?;
    let bytes = read(&body, descriptor)?;
    let descriptor = Document::parse(&bytes)?;
    let Some(original) = find_original(&body, &descriptor)? else {
        return require_entxml();
    };
    verify_target(&archive, &original)?;
    let (mut placements, uses, body_xml, entity_xml) = with_entry_capture(&archive, |capture| {
        let placements =
            Placements::read(&archive, None, capture, PlacementScope::RecordSelection)?;
        let uses = capture.source_uses();
        Ok((
            placements,
            uses.clone(),
            original_descriptor_ref(&archive, witness.body, witness.body_member, uses)?,
            original_descriptor_ref(&archive, witness.body, original.member, uses)?,
        ))
    })?;
    let inventory = uses.freeze()?;
    let uses: HashMap<_, _> = inventory
        .uses()
        .iter()
        .map(|usage| (usage.id().as_str(), usage))
        .collect();
    let parent_route: Vec<_> = witness.body_route.iter().map(|node| node.index).collect();
    let root = u32::try_from(witness.system.index())?;
    let rows = placements.take_locations();
    let mut matches = rows
        .rows
        .iter()
        .filter(|row| row.matches_body_child(root, &parent_route, original.child));
    let row = matches
        .next()
        .ok_or("the original typed location occurrence is projected")?;
    assert!(
        matches.next().is_none(),
        "one Child in the original body occurrence"
    );
    let row = serde_json::to_value(row)?;
    verify_link(&row, &original, &body_xml, &entity_xml, &uses)?;
    watch.check()?;
    Ok(())
}

fn verify_link(
    row: &Value,
    original: &Original,
    body_xml: &PhysicalSourceRef,
    entity_xml: &PhysicalSourceRef,
    uses: &HashMap<&str, &SourceUseDescription>,
) -> TestResult {
    let link = row
        .pointer("/declaration/entXml")
        .ok_or("original EntXML lookup")?;
    assert_eq!(
        link["inventoryComplete"], true,
        "the selected physical inventory is complete"
    );
    assert_eq!(
        link["selected"]["value"],
        json!({"entryIndex": original.member, "inputRef": entity_xml}),
        "selected original Entry"
    );
    assert_attribute(&link["selected"], uses, body_xml, original.child, "guid")?;
    assert_attribute(
        &link["selected"],
        uses,
        entity_xml,
        original.root,
        "EntityCryGUID",
    )?;
    let data = link
        .pointer("/candidates/value/0/document/value")
        .ok_or("original Entity candidate")?;
    verify_elements(data, original, uses, entity_xml)
}

struct Original {
    child: u32,
    member: usize,
    root: u32,
    owner: Guid,
    configs: Vec<(u32, Guid)>,
    areas: Vec<(u32, Guid)>,
}

fn find_original(body: &Archive, descriptor: &Document<'_>) -> TestResult<Option<Original>> {
    let locations = typed_locations(descriptor);
    for member in body.entries() {
        let parts = member.path().rsplit(['/', '\\']);
        if !member.path().to_ascii_lowercase().ends_with(".entxml")
            || !parts.skip(1).any(|p| p.eq_ignore_ascii_case("entdata"))
        {
            continue;
        }
        let bytes = read(body, member)?;
        let xml = Document::parse(&bytes)?;
        let root = xml.root();
        let Some(owner) = parsed_guid(root, "EntityCryGUID") else {
            continue;
        };
        let Some(&child) = locations.get(&owner) else {
            continue;
        };
        let configs = original_fields(
            &xml,
            root.children()
                .filter(|node| node.name() == "HarvestableComponent"),
            "SubConfigGUID",
        )?;
        if configs.is_empty() {
            continue;
        }
        let area_entities = root
            .children()
            .filter(|node| node.name() == "AreaSphereComponent")
            .flat_map(|node| node.children().filter(|list| list.name() == "Entities"))
            .flat_map(|node| node.children().filter(|entity| entity.name() == "Entity"));
        let areas = original_fields(&xml, area_entities, "EntityCryGUID")?;
        if !areas.is_empty() {
            return Ok(Some(Original {
                child,
                member: member.index(),
                root: xml.original_index(root).ok_or("original EntXML root")?,
                owner,
                configs,
                areas,
            }));
        }
    }
    Ok(None)
}

fn original_fields<'a>(
    xml: &Document<'_>,
    nodes: impl Iterator<Item = Root<'a>>,
    field: &str,
) -> TestResult<Vec<(u32, Guid)>> {
    nodes
        .map(|node| {
            Ok((
                xml.original_index(node).ok_or("original EntXML node")?,
                parsed_guid(node, field).ok_or("authored Entity GUID field")?,
            ))
        })
        .collect()
}

fn verify_target(archive: &Archive, original: &Original) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    for (_, config) in &original.configs {
        let record = forge
            .record_by_guid(*config)
            .ok_or("authored configuration Record")?;
        assert_eq!(record.type_name()?, "SubHarvestableMultiConfigRecord");
    }
    Ok(())
}

fn verify_elements(
    data: &Value,
    original: &Original,
    uses: &HashMap<&str, &SourceUseDescription>,
    entity_xml: &PhysicalSourceRef,
) -> TestResult {
    assert_eq!(data["rootGuid"]["value"], json!(original.owner.to_string()));
    let components = data["harvestableComponents"]["value"]
        .as_array()
        .ok_or("root component occurrences")?;
    assert_eq!(components.len(), original.configs.len());
    for (component, &(index, guid)) in components.iter().zip(&original.configs) {
        assert_eq!(component["nodeIndex"]["value"], index);
        assert_eq!(
            component["ownerGuid"]["value"],
            json!(original.owner.to_string())
        );
        assert_eq!(component["subConfigGuid"]["value"], json!(guid.to_string()));
        assert_attribute(
            &component["subConfigGuid"],
            uses,
            entity_xml,
            index,
            "SubConfigGUID",
        )?;
    }
    let areas = data["areaEntities"]["value"]
        .as_array()
        .ok_or("original area occurrence list")?;
    assert_eq!(areas.len(), original.areas.len());
    for (area, &(index, guid)) in areas.iter().zip(&original.areas) {
        assert_eq!(area["nodeIndex"]["value"], index);
        assert_eq!(area["guid"]["value"], json!(guid.to_string()));
        assert_attribute(&area["guid"], uses, entity_xml, index, "EntityCryGUID")?;
    }
    Ok(())
}

fn typed_locations(xml: &Document<'_>) -> HashMap<Guid, u32> {
    let mut locations = HashMap::new();
    let mut pending = vec![xml.root()];
    while let Some(node) = pending.pop() {
        pending.extend(node.children());
        if node.name() == "Child"
            && attribute(node, "locationType").is_some()
            && let Some(guid) = parsed_guid(node, "guid")
            && let Some(index) = xml.original_index(node)
        {
            locations.insert(guid, index);
        }
    }
    locations
}

fn parsed_guid(node: Root<'_>, name: &str) -> Option<Guid> {
    attribute(node, name)?.parse().ok()
}

fn read(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn require_entxml() -> TestResult {
    if std::env::var_os("P4KX_EXPECT_STARCHITECT").is_none() {
        Ok(())
    } else {
        Err(
            "the installed Archive must declare an exact location and Harvestable EntXML join"
                .into(),
        )
    }
}
