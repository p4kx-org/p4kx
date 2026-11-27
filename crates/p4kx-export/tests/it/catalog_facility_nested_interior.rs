// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_extraction::cryxml::{SourceDocument, SourceReadOptions};
use p4kx_extraction::{Archive, PathIndex};
use serde_json::json;

use super::{TestResult, build, fixture, interior_rows, proof};

#[test]
fn catalog_follows_authored_child_occurrences_and_composes_room_local_transforms() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::NestedChildren)?;
    check_original_children(&archive)?;
    let catalog = build(&archive)?;
    let rows = interior_rows(&catalog)?;
    assert_eq!(
        rows.len(),
        6,
        "two direct entities and two occurrences of the two original child entities"
    );
    let [_direct, _direct_other, first, _, second, _] = rows.as_slice() else {
        return Err("two independent declared Child occurrences".into());
    };
    check_route(first, 1, 1.0)?;
    check_route(second, 2, 9.0)?;
    let [coverage] = proof::rows(&catalog, "/placements/facilityInteriorCoverage")? else {
        return Err("one selected original root RoomAssociation".into());
    };
    check_occurrences(coverage)?;
    let cycles: Vec<_> = proof::rows(coverage, "/findings")?
        .iter()
        .filter(|finding| finding["kind"] == "childCycle")
        .map(|finding| &finding["containerOrdinal"])
        .collect();
    assert_eq!(
        cycles,
        [&json!(1), &json!(2)],
        "each repeated route identifies its own ancestor cycle"
    );
    proof::closure(&catalog)
}

#[test]
fn catalog_preserves_local_facilities_when_child_pose_is_invalid() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::ChildFaults)?;
    let catalog = build(&archive)?;
    let rows = interior_rows(&catalog)?;
    let [_root, _root_other, child, _child_other] = rows.as_slice() else {
        return Err("one readable Child supplies both local Entity slots".into());
    };
    let source = proof::at(child, "/source")?;
    proof::present(
        source,
        "localTransform",
        &json!([
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 2.0],
            [0.0, 0.0, 1.0, 3.0]
        ]),
        Some(true),
    )?;
    let transform = proof::cell(source, "rootRoomTransform", "invalid")?;
    assert_eq!(
        transform["containsEncryptedSource"], true,
        "invalid composition still identifies the original encrypted inputs"
    );
    let [coverage] = proof::rows(&catalog, "/placements/facilityInteriorCoverage")? else {
        return Err("the original root room coverage".into());
    };
    check_faults(coverage)?;
    proof::closure(&catalog)
}

fn check_faults(coverage: &serde_json::Value) -> TestResult {
    let [_root, child] = proof::rows(coverage, "/containers")? else {
        return Err("only the original root and one unique Child opened".into());
    };
    let original = proof::at(child, "/child")?;
    proof::cell(original, "localTransform", "invalid")?;
    let attributes = proof::rows(original, "/attributes")?;
    let position = attributes
        .iter()
        .find(|attribute| attribute.pointer("/name/value") == Some(&json!("pos")))
        .ok_or("original malformed position")?;
    proof::present(position, "value", &json!("bad,3,4"), Some(true))?;
    let findings = proof::rows(coverage, "/findings")?;
    for (kind, ordinal) in [
        ("invalidChildTransform", 1),
        ("ambiguousContainer", 0),
        ("invalidChildPath", 0),
        ("ambiguousChildName", 0),
    ] {
        assert!(
            findings
                .iter()
                .any(|finding| finding["kind"] == kind && finding["containerOrdinal"] == ordinal),
            "{kind} is retained for the original route at occurrence {ordinal}"
        );
    }
    let duplicate = findings
        .iter()
        .find(|finding| finding["kind"] == "ambiguousChildName")
        .ok_or("duplicate original name finding")?;
    let source = proof::at(duplicate, "/child")?;
    proof::cell(source, "name", "invalid")?;
    assert_eq!(
        proof::rows(source, "/attributes")?.len(),
        2,
        "both original attribute slots remain available after the ambiguity"
    );
    Ok(())
}

fn check_occurrences(coverage: &serde_json::Value) -> TestResult {
    let [root, first, second] = proof::rows(coverage, "/containers")? else {
        return Err("one original root and two Child occurrences".into());
    };
    assert_eq!(
        root["parentOrdinal"],
        serde_json::Value::Null,
        "root has no Child parent"
    );
    for row in [first, second] {
        assert_eq!(
            row["parentOrdinal"], 0,
            "the Child joins its root-room occurrence"
        );
        proof::present(
            proof::at(row, "/child")?,
            "name",
            &json!(fixture::CHILD),
            Some(true),
        )?;
    }
    assert_eq!(
        proof::at(first, "/container/value")?,
        proof::at(second, "/container/value")?,
        "the two declarations share one actual nested Entry"
    );
    assert_eq!(
        proof::at(first, "/container/value/entryIndex")?,
        &json!(3),
        "the physical inner SOCpak shadows the unrelated outer candidate"
    );
    assert_ne!(
        proof::at(first, "/child/position/value/descriptorNodeIndex")?,
        proof::at(second, "/child/position/value/descriptorNodeIndex")?,
        "each Child declaration retains its original descriptor node"
    );
    Ok(())
}

fn check_route(row: &serde_json::Value, ordinal: u32, x: f32) -> TestResult {
    assert_eq!(
        proof::at(row, "/source/position/value/containerOccurrenceOrdinal")?,
        &json!(ordinal),
        "the facility joins its exact Child occurrence"
    );
    let source = proof::at(row, "/source")?;
    proof::present(
        source,
        "rootRoomTransform",
        &json!([
            [-1.0, 0.0, 0.0, x],
            [0.0, -1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 7.0]
        ]),
        Some(true),
    )?;
    Ok(())
}

fn check_original_children(archive: &Archive) -> TestResult {
    let root = PathIndex::new(archive)
        .find(&format!(r"Data\{}", fixture::ROOM))
        .ok_or("the physical room SOCpak")?;
    let opened = archive.open_container(root)?;
    let index = PathIndex::new(&opened);
    let descriptor = index
        .find(fixture::DESCRIPTOR)
        .ok_or("the root descriptor")?;
    let mut original = Vec::new();
    opened.open_entry(descriptor)?.read_to_end(&mut original)?;
    let xml = SourceDocument::parse(&original, SourceReadOptions::default())?;
    let child_names: Vec<_> = xml
        .root()
        .elements()
        .find(|section| section.name() == "ChildObjectContainers")
        .ok_or("the original Child list")?
        .elements()
        .filter_map(|child| child.attribute("name"))
        .collect();
    assert_eq!(
        child_names,
        [fixture::CHILD, fixture::CHILD],
        "the authored descriptor repeats a physical child name in two nodes"
    );
    let child = index
        .find(&format!("{}.socpak", fixture::CHILD))
        .ok_or("the actual nested SOCpak shadows the unrelated outer candidate")?;
    let nested = opened.open_container(child)?;
    assert!(
        PathIndex::new(&nested)
            .find(fixture::CHILD_STREAM)
            .is_some(),
        "the selected nested Archive contains the actual authored stream"
    );
    Ok(())
}
