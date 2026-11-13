// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_export::build_catalog;
use p4kx_extraction::model::{SocEntity, SocEntityFile};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, PathIndex, Value as Original};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_facility_interior_fixture.rs"]
mod fixture;
#[path = "catalog_facility_nested_interior.rs"]
mod nested;

#[test]
fn catalog_links_original_soc_entity_slots_to_their_physical_facility_owner() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::Direct)?;
    verify_original(&archive)?;
    let catalog = build(&archive)?;
    let [configuration] = proof::rows(&catalog, "/configurations/facilities/rows")? else {
        return Err("one actual physical seat configuration".into());
    };
    proof::present(
        configuration,
        "ownerGuid",
        &json!(common::guid(fixture::SEAT)),
        Some(false),
    )?;
    let interior = interior_rows(&catalog)?;
    assert_eq!(
        interior.len(),
        2,
        "each original SOC Entity slot links to the facility owner through its actual room"
    );
    check_positions(&interior)?;
    check_transforms(&interior)?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)
}

#[test]
fn catalog_records_partial_direct_room_coverage_and_ambiguous_sources() -> TestResult {
    for (case, count, finding) in [
        (fixture::Case::MissingStreamAndChild, 2, "missingStream"),
        (fixture::Case::AmbiguousStream, 0, "ambiguousStream"),
        (fixture::Case::AmbiguousClass, 0, "ambiguousClass"),
        (fixture::Case::LossyDescriptor, 0, "unreadableDescriptor"),
        (
            fixture::Case::UnexpectedDescriptorRoot,
            0,
            "unreadableDescriptor",
        ),
        (fixture::Case::LossyChunk, 2, "lossyEntityChunk"),
    ] {
        let (_folder, archive) = fixture::archive(case)?;
        let catalog = build(&archive)?;
        let rows = interior_rows(&catalog)?;
        assert_eq!(
            rows.len(),
            count,
            "only actual unique room sources and class joins count"
        );
        let [coverage] = proof::rows(&catalog, "/placements/facilityInteriorCoverage")? else {
            return Err("one selected room coverage".into());
        };
        let findings = proof::rows(coverage, "/findings")?;
        assert!(
            findings.iter().any(|row| row["kind"] == finding),
            "room inspection records {finding}"
        );
        if matches!(case, fixture::Case::MissingStreamAndChild) {
            let missing = findings
                .iter()
                .find(|row| row["kind"] == "missingStream")
                .ok_or("the authored missing stream declaration")?;
            proof::present(missing, "stream", &json!("missing.soc"), Some(true))?;
            let child = findings
                .iter()
                .find(|row| row["kind"] == "missingContainer")
                .ok_or("missing actual Child source")?;
            proof::present(
                proof::at(child, "/child")?,
                "name",
                &json!("missing_child"),
                Some(true),
            )?;
        }
        if matches!(case, fixture::Case::LossyChunk) {
            let placement = rows
                .first()
                .ok_or("a placement with repaired XML evidence")?;
            let class = proof::at(placement, "/source/classRecordIndex")?;
            assert_eq!(
                class["sourceSetComplete"], false,
                "the selected physical class survives with incomplete repaired XML evidence"
            );
        }
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn catalog_keeps_repeated_stream_declarations_at_distinct_original_nodes() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::RepeatedStream)?;
    let catalog = build(&archive)?;
    let rows = interior_rows(&catalog)?;
    let [first, second, again_first, again_second] = rows.as_slice() else {
        return Err("two physical entities for each authored stream occurrence".into());
    };
    let first = proof::at(first, "/source/position/value")?;
    let second = proof::at(second, "/source/position/value")?;
    let again_first = proof::at(again_first, "/source/position/value")?;
    let again_second = proof::at(again_second, "/source/position/value")?;
    assert_eq!(
        first["stream"], again_first["stream"],
        "both declarations target one physical stream"
    );
    assert_eq!(
        first["entityOrdinal"], again_first["entityOrdinal"],
        "first original entity occurs twice"
    );
    assert_eq!(
        second["entityOrdinal"], again_second["entityOrdinal"],
        "second original entity occurs twice"
    );
    assert_ne!(
        first["descriptorNodeIndex"], again_first["descriptorNodeIndex"],
        "OC declarations have independent original nodes"
    );
    proof::closure(&catalog)
}

fn build(archive: &Archive) -> TestResult<Value> {
    Ok(serde_json::from_slice(
        build_catalog(archive, &common::options())?.json(),
    )?)
}

fn interior_rows(catalog: &Value) -> TestResult<Vec<&Value>> {
    Ok(proof::rows(catalog, "/placements/facilityInstances/rows")?
        .iter()
        .filter(|row| row.pointer("/source/kind") == Some(&json!("interior")))
        .collect())
}

fn check_positions(rows: &[&Value]) -> TestResult {
    for (row, ordinal) in rows.iter().zip([0, 2]) {
        let source = proof::at(row, "/source")?;
        proof::present(source, "streamName", &json!(fixture::STREAM), Some(true))?;
        proof::present(source, "classRecordIndex", &json!(1), Some(true))?;
        let room = proof::at(source, "/room")?;
        proof::present(room, "objectContainerOrdinal", &json!(0), Some(false))?;
        let position = proof::cell(source, "position", "present")?;
        assert_eq!(
            position["containsEncryptedSource"], true,
            "position uses the encrypted SOCpak ancestry"
        );
        assert_eq!(
            proof::at(position, "/value/chunkIndex")?,
            &json!(1),
            "physical CrCh table index"
        );
        assert_eq!(
            proof::at(position, "/value/entityOrdinal")?,
            &json!(ordinal),
            "original Entity slot survives intervening metadata"
        );
        assert_eq!(
            proof::at(position, "/value/container/entryIndex")?,
            &json!(1),
            "original outer directory entry"
        );
        assert_eq!(
            proof::at(position, "/value/descriptor/entryIndex")?,
            &json!(0),
            "original descriptor entry"
        );
        assert_eq!(
            proof::at(position, "/value/stream/entryIndex")?,
            &json!(1),
            "original declared stream entry"
        );
        let owner = proof::at(row, "/configurations/0/owner")?;
        proof::present(owner, "ownerRecordIndex", &json!(1), Some(false))?;
        proof::present(owner, "componentOrdinal", &json!(1), Some(false))?;
    }
    Ok(())
}

fn check_transforms(rows: &[&Value]) -> TestResult {
    let [valid, invalid] = rows else {
        return Err("valid and malformed original entity".into());
    };
    proof::present(
        proof::at(valid, "/source")?,
        "localTransform",
        &json!([
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 2.0],
            [0.0, 0.0, 1.0, 3.0]
        ]),
        Some(true),
    )?;
    let source = proof::at(invalid, "/source")?;
    let cell = proof::cell(source, "localTransform", "invalid")?;
    assert_eq!(
        cell["containsEncryptedSource"], true,
        "failed transform still identifies the original encrypted contributor"
    );
    let attribute = proof::rows(source, "/entityAttributes")?
        .iter()
        .find(|attribute| attribute.pointer("/name/value") == Some(&json!("Pos")))
        .ok_or("the original failed position attribute")?;
    proof::present(attribute, "value", &json!("bad,0,0"), Some(true))?;
    Ok(())
}

fn verify_original(archive: &Archive) -> TestResult {
    let dcb = DataForgeEntry::read(archive)?;
    let forge = dcb.open()?;
    verify_room_declaration(&forge)?;
    let outer = PathIndex::new(archive)
        .find(&format!(r"Data\{}", fixture::ROOM))
        .ok_or("actual outer SOCpak entry")?;
    let container = archive.open_container(outer)?;
    let inner = PathIndex::new(&container);
    let descriptor = inner
        .find(fixture::DESCRIPTOR)
        .ok_or("actual descriptor entry")?;
    let mut descriptor_bytes = Vec::new();
    container
        .open_entry(descriptor)?
        .read_to_end(&mut descriptor_bytes)?;
    let descriptor = p4kx_extraction::cryxml::SourceDocument::parse(
        &descriptor_bytes,
        p4kx_extraction::cryxml::SourceReadOptions::default(),
    )?;
    let stream = descriptor
        .root()
        .elements()
        .find(|node| node.name() == "StreamingOCs")
        .and_then(|node| node.elements().next())
        .and_then(|node| node.attribute("name"))
        .ok_or("actual authored OC selector")?;
    assert_eq!(
        stream,
        fixture::STREAM,
        "the descriptor owns the original stream selector"
    );
    let mut bytes = Vec::new();
    container
        .open_entry(inner.find(stream).ok_or("selected original stream")?)?
        .read_to_end(&mut bytes)?;
    let source = SocEntityFile::parse(&bytes)?;
    let chunk = source
        .entity_chunks()
        .next()
        .ok_or("actual original entity chunk")??;
    let actual = forge
        .record(1)
        .ok_or("actual target Entity Record")?
        .guid()
        .to_string();
    let slots: Vec<_> = chunk
        .entities()
        .filter(|entity| entity.element().attribute("EntityClassGUID") == Some(actual.as_str()))
        .map(SocEntity::ordinal)
        .collect();
    assert_eq!(
        slots,
        [0, 2],
        "the original clear DCB and two encrypted SOC slots name the same class"
    );
    Ok(())
}

fn verify_room_declaration(forge: &DataForge<'_>) -> TestResult {
    let ship = forge.record(0).ok_or("original ship")?.tree()?;
    let Some(Original::Array(components)) = ship.field("Components") else {
        return Err("original ship Components array".into());
    };
    let Some(Original::Struct(vehicle)) = components.first() else {
        return Err("physical vehicle component".into());
    };
    let Some(Original::Array(rooms)) = vehicle.field("objectContainers") else {
        return Err("original objectContainers array".into());
    };
    let Some(Original::Struct(room)) = rooms.first() else {
        return Err("physical room slot".into());
    };
    assert!(
        matches!(room.field("fileName"), Some(Original::String(name)) if *name == fixture::ROOM),
        "the physical vehicle component names the actual nested archive"
    );
    Ok(())
}
