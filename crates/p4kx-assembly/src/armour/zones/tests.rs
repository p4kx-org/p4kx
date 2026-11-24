// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{
    BodyVariant, CLOTHING, ClothingZoneCoverage, MAX_CHUNKS, ZoneChunk, ZoneList, covered,
    read_zones, undecided,
};

fn chunk(zone: &str, layer: u32, visible: &[u32], conditional: bool) -> ZoneChunk {
    ZoneChunk {
        zone: zone.to_owned(),
        layer,
        visible_layers: visible.to_vec(),
        conditional,
        incomplete: false,
        position: None,
    }
}

fn observed_zones(bytes: &[u8]) -> Result<ZoneList, Box<dyn Error>> {
    let forge = DataForge::open(bytes)?;
    let tree = forge
        .records()
        .next()
        .ok_or("clothing fixture is missing")?
        .tree()?;
    Ok(read_zones(std::iter::once((0, tree.root())), None))
}

#[test]
fn an_invalid_chunk_field_differs_from_a_valid_empty_declaration() -> Result<(), Box<dyn Error>> {
    let builder = || {
        DataForgeBuilder::new()
            .structure(TestStruct::new("Chunk").field("MeshChunk", TestType::String))
            .record(TestRecord::new(1, CLOTHING, "Clothing", "clothing.xml"))
    };
    let empty = builder()
        .structure(TestStruct::new(CLOTHING).array("Chunks", TestType::Class("Chunk".into())))
        .build();
    let invalid = builder()
        .structure(TestStruct::new(CLOTHING).field("Chunks", TestType::Int32))
        .build();
    let empty = observed_zones(&empty)?;
    let invalid = observed_zones(&invalid)?;
    assert!(empty.chunks.is_empty());
    assert!(invalid.chunks.is_empty());
    assert!(!empty.incomplete);
    assert!(invalid.incomplete);
    Ok(())
}

#[test]
fn malformed_owned_rows_cannot_supply_definite_modular_coverage() -> Result<(), Box<dyn Error>> {
    let chunk = |zone, conditions| {
        TestValue::Struct(
            TestInstance::new("Chunk")
                .value("MeshChunk", TestValue::text(zone))
                .value("Layer", TestValue::Int32(1))
                .value("VisibilityConditions", TestValue::Array(conditions)),
        )
    };
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Condition").field("Unused", TestType::Int32))
        .structure(
            TestStruct::new("Chunk")
                .field("MeshChunk", TestType::String)
                .field("Layer", TestType::Int32)
                .array(
                    "VisibilityConditions",
                    TestType::StrongPointer("Condition".into()),
                ),
        )
        .structure(
            TestStruct::new(CLOTHING).array("Chunks", TestType::StrongPointer("Chunk".into())),
        )
        .record(
            TestRecord::new(1, CLOTHING, "Clothing", "clothing.xml").value(
                "Chunks",
                TestValue::Array(vec![
                    TestValue::Null,
                    chunk("torso01_zone", vec![TestValue::Null]),
                    chunk("torso02_zone", Vec::new()),
                ]),
            ),
        )
        .build();
    let observed = observed_zones(&bytes)?;
    assert!(observed.incomplete);
    assert!(!observed.truncated);
    assert_eq!(
        covered(observed.chunks.iter()),
        BTreeSet::from(["torso01_zone", "torso02_zone"])
    );
    let coverage = ClothingZoneCoverage::of(observed.known_chunks(), BodyVariant::Male);
    assert_eq!(*coverage.covered(), BTreeSet::from(["torso02_zone"]));
    assert!(coverage.undecided().is_empty());
    Ok(())
}

#[test]
fn only_retained_clothing_chunks_keep_source_coordinates() -> Result<(), Box<dyn Error>> {
    let chunk = |zone| {
        TestValue::Struct(
            TestInstance::new("Chunk")
                .value("MeshChunk", TestValue::text(zone))
                .value("Layer", TestValue::Int32(1)),
        )
    };
    let component = |chunks| {
        TestValue::Struct(TestInstance::new(CLOTHING).value("Chunks", TestValue::Array(chunks)))
    };
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Chunk")
                .field("MeshChunk", TestType::String)
                .field("Layer", TestType::Int32),
        )
        .structure(
            TestStruct::new(CLOTHING).array("Chunks", TestType::StrongPointer("Chunk".into())),
        )
        .structure(
            TestStruct::new("Owner").array("Components", TestType::StrongPointer(CLOTHING.into())),
        )
        .record(
            TestRecord::new(1, "Owner", "Clothing", "clothing.xml").value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Null,
                    component(vec![chunk("torso01_zone"); MAX_CHUNKS + 1]),
                    component(vec![chunk("torso02_zone")]),
                ]),
            ),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .records()
        .next()
        .ok_or("clothing fixture is missing")?
        .tree()?;
    let observed = read_zones(
        crate::read::indexed_struct_items(tree.root(), "Components"),
        None,
    );
    assert!(observed.truncated);
    assert_eq!(observed.chunks.len(), MAX_CHUNKS);
    assert!(
        observed
            .chunks
            .iter()
            .enumerate()
            .all(|(row, chunk)| chunk.position == Some((1, row))),
        "only coordinates from the retained rows of the first clothing component are admitted"
    );
    assert_eq!(
        covered(observed.known_chunks()),
        BTreeSet::from(["torso01_zone"])
    );
    Ok(())
}

#[test]
fn a_chunk_above_the_body_covers_its_zone() {
    let chunks = [chunk("torso01_zone", 1, &[], false)];
    assert_eq!(
        covered(&chunks).into_iter().collect::<Vec<_>>(),
        ["torso01_zone"]
    );
}

#[test]
fn a_chunk_that_shows_the_body_through_covers_nothing() {
    let chunks = [chunk("l_arm04_zone", 1, &[0], false)];
    assert!(covered(&chunks).is_empty());
}

#[test]
fn the_body_is_covered_when_any_chunk_above_it_hides_it() {
    let chunks = [
        chunk("torso01_zone", 1, &[], false),
        chunk("torso01_zone", 2, &[0, 1], false),
    ];
    assert_eq!(covered(&chunks).len(), 1);
}

#[test]
fn chunks_on_the_body_layer_and_conditional_ones_cover_nothing() {
    let chunks = [
        chunk("torso01_zone", 0, &[], false),
        chunk("torso02_zone", 2, &[], true),
    ];
    assert!(covered(&chunks).is_empty());
}

#[test]
fn a_conditional_chunk_on_a_body_zone_is_undecided_unless_another_covers_it() {
    let chunks = [
        chunk("hips_zone", 2, &[], true),
        chunk("torso02_zone", 2, &[], true),
        chunk("torso01_zone", 2, &[], true),
        chunk("torso01_zone", 1, &[], false),
        chunk("theta_core_zone", 2, &[], true),
        chunk("l_arm01_zone", 2, &[0], true),
    ];
    let covered = covered(&chunks);
    let open = undecided(&chunks, BodyVariant::Male, &covered);
    assert_eq!(open.into_iter().collect::<Vec<_>>(), ["torso02_zone"]);
}
