// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A sub-geometry the port's flags choose that is not in the Archive gives way to the main
//! geometry of the Entity.

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestModel, TestPort, entity_schema};

use crate::assembly_support::{TestResult, fixture};

const SHIP: u128 = 0x1000;
const SKID: u128 = 0x2000;

fn entries() -> Vec<TestEntry> {
    let skid = TestEntity::new(SKID, "TEST_Skid", "entities/test_skid.xml")
        .geometry("Objects/Test/cube.cgf", None)
        .geometry_variant("right", "Objects/Test/absent.cgf", None)
        .geometry_variant("left", "Objects/Test/cube.cgf", None);
    let ship = TestEntity::new(SHIP, "TEST_Ship", "entities/test_ship.xml")
        .port(
            TestPort::new("skid_right")
                .flags("right")
                .default_item(SKID),
        )
        .port(TestPort::new("skid_left").flags("left").default_item(SKID));
    let database = entity_schema(DataForgeBuilder::new())
        .record(skid.record())
        .record(ship.record());
    let model = TestModel::cube();
    vec![
        TestEntry::zstd("Data\\Game2.dcb", database.build()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgf", model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgfm", model.cgfm()),
    ]
}

fn assemble(archive: &Archive) -> Result<Assembly, Box<dyn std::error::Error>> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find("TEST_Ship")?;
    let tree = resolver.resolve(&record)?;
    let models = ArchiveModels::new(archive);
    Ok(Assembly::build(
        &tree,
        &models,
        &AssemblyOptions::default(),
    )?)
}

#[test]
fn a_missing_sub_geometry_falls_back_to_the_main_geometry_with_a_note() -> TestResult {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;

    let assembly = assemble(&archive)?;

    let right = assembly
        .part_at("skid_right")
        .ok_or("the part of the right port")?;
    assert!(right.model().is_some(), "the main geometry is drawn");
    assert_eq!(right.geometry(), Some("Objects/Test/cube.cgf"));
    let codes: Vec<(&str, &str, bool)> = assembly
        .issues()
        .iter()
        .map(|issue| (issue.path(), issue.kind().code(), issue.kind().skips()))
        .collect();
    assert!(
        codes.contains(&("skid_right", "sub_geometry_missing", false)),
        "{codes:?}"
    );
    assert!(
        !codes.iter().any(|(_, code, _)| *code == "geometry_missing"),
        "{codes:?}"
    );
    let left = assembly
        .part_at("skid_left")
        .ok_or("the part of the left port")?;
    assert!(left.model().is_some(), "the selected variant is drawn");
    assert_eq!(
        right.geometry(),
        left.geometry(),
        "the same path has two source routes"
    );
    for (part, field) in [
        (right, "Components[0].Geometry.Geometry.Geometry.path"),
        (
            left,
            "Components[0].Geometry.SubGeometry[1].Geometry.Geometry.path",
        ),
    ] {
        assert!(
            matches!(part.geometry_selector(),
                Some(SourceLocation::DataForge { field: observed, .. }) if observed == field),
            "the selected geometry retains its own authored route: {part:?}"
        );
    }
    assert_eq!(
        codes
            .iter()
            .filter(|(path, ..)| *path == "skid_left")
            .count(),
        0,
        "{codes:?}"
    );
    Ok(())
}
