// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::io::Read as _;

use p4kx_extraction::{Archive, ArchiveWatch, Entry, Guid, PhysicalSourceRef};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::catalog_read::original::with_entry_capture;
use crate::starmap_containers::{PlacementScope, Placements};
use crate::starmap_xml::{Document, Node, attribute};

use super::{TestResult, assert_attribute, original_descriptor_ref, source};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_location_context_matches_original_companion_and_field_sources() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let Some(witness) = source::find_witness(&archive, &watch)? else {
        return require_context();
    };
    let body = archive.open_container(witness.body)?;
    let descriptor = body
        .entries()
        .get(witness.body_member)
        .ok_or("original body descriptor")?;
    let bytes = read(&body, descriptor)?;
    let xml = Document::parse(&bytes)?;
    let Some(original) = find_context(&body, &xml)? else {
        return require_context();
    };
    let (mut placements, uses, body_xml, context_xml) = with_entry_capture(&archive, |capture| {
        let placements =
            Placements::read(&archive, None, capture, PlacementScope::RecordSelection)?;
        let uses = capture.source_uses();
        Ok((
            placements,
            uses.clone(),
            original_descriptor_ref(&archive, witness.body, witness.body_member, uses)?,
            original_descriptor_ref(&archive, witness.body, original.entry, uses)?,
        ))
    })?;
    let inventory = uses.freeze()?;
    let uses: HashMap<_, _> = inventory
        .uses()
        .iter()
        .map(|item| (item.id().as_str(), item))
        .collect();
    let rows = placements.take_locations();
    let parent_route: Vec<_> = witness.body_route.iter().map(|node| node.index).collect();
    let root = u32::try_from(witness.system.index())?;
    let mut matches = rows
        .rows
        .iter()
        .filter(|row| row.matches_body_child(root, &parent_route, original.child));
    let row = matches
        .next()
        .ok_or("the physical Child belongs to its original body occurrence")?;
    assert!(
        matches.next().is_none(),
        "one Child in the original body occurrence"
    );
    let row = serde_json::to_value(row)?;
    verify(&row, &original, &body_xml, &context_xml, &uses)?;
    watch.check()?;
    Ok(())
}

struct Original {
    child: u32,
    entry: usize,
    node: u32,
    preset: Guid,
    record_name: Option<String>,
}

fn find_context(archive: &Archive, xml: &Document<'_>) -> TestResult<Option<Original>> {
    let mut entries: HashMap<Guid, Vec<&Entry>> = HashMap::new();
    for entry in archive.entries() {
        let path = entry.path().to_ascii_lowercase();
        let mut parts = path.rsplit(['/', '\\']);
        if let Some(stem) = parts.next().and_then(|file| file.strip_suffix(".xml"))
            && parts.any(|part| part == "hcd")
            && let Ok(guid) = stem.parse()
        {
            entries.entry(guid).or_default().push(entry);
        }
    }
    let mut pending = vec![xml.root()];
    while let Some(node) = pending.pop() {
        pending.extend(node.children());
        if node.name() != "Child"
            || attribute(node, "locationType").is_none()
            || !matches!(
                attribute(node, "hasHierarchicalContextData"),
                Some("1" | "true")
            )
        {
            continue;
        }
        let Some([entry]) = attribute(node, "guid")
            .and_then(|raw| raw.parse::<Guid>().ok())
            .and_then(|guid| entries.get(&guid))
            .map(Vec::as_slice)
        else {
            continue;
        };
        let bytes = read(archive, entry)?;
        let header = Document::parse(&bytes)?;
        let root = header.root();
        if root.name() != "HierarchicalContextData" {
            continue;
        }
        let Some(preset) = attribute(root, "preset").and_then(|raw| raw.parse().ok()) else {
            continue;
        };
        return Ok(Some(Original {
            child: xml.original_index(node).ok_or("original Child index")?,
            entry: entry.index(),
            node: header
                .original_index(root)
                .ok_or("original companion root")?,
            preset,
            record_name: attribute(root, "presetRecordName").map(str::to_owned),
        }));
    }
    Ok(None)
}

fn verify(
    row: &Value,
    original: &Original,
    body: &PhysicalSourceRef,
    companion: &PhysicalSourceRef,
    uses: &HashMap<&str, &p4kx_extraction::SourceUseDescription>,
) -> TestResult {
    let context = row
        .pointer("/declaration/hierarchicalContext")
        .ok_or("sourced context lookup")?;
    let candidate = context
        .pointer("/candidates/value/0/value")
        .ok_or("one original physical member")?;
    assert!(
        candidate == &json!({ "inputRef": companion, "entryIndex": original.entry }),
        "lookup returns the physical Entry selected by the Child"
    );
    let preset = context
        .pointer("/document/value/presetGuid")
        .ok_or("authored context preset")?;
    assert!(
        preset.get("value") == Some(&json!(original.preset.to_string())),
        "the parsed preset matches the original XML"
    );
    assert!(
        context.pointer("/document/value/presetRecordName/value")
            == Some(&json!(original.record_name)),
        "the authored name is retained independently"
    );
    assert_attribute(preset, uses, companion, original.node, "preset")?;
    assert_attribute(
        &context["document"],
        uses,
        body,
        original.child,
        "hasHierarchicalContextData",
    )?;
    Ok(())
}

fn read(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn require_context() -> TestResult {
    if std::env::var_os("P4KX_EXPECT_STARCHITECT").is_none() {
        Ok(())
    } else {
        Err("the installed archive must declare a readable location-context companion".into())
    }
}
