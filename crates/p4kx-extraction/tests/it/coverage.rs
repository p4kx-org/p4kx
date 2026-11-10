// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The coverage check of a reference list over a synthetic Archive.

use std::error::Error;

use p4kx_extraction::{Archive, CoverageReport, DataForgeEntry, ReferenceKind, ReferenceList};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntry,
    TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

const LIST: &str = "missions record MissionItem
missions candidate MissionLocationTemplate
reputation record Faction
reputation record Loop
items component WeaponParams
items component MissileParams
items component MissingParams
starmap starmap-type SolarSystem
starmap starmap-type Nowhere
vehicles entries Data/Vehicles/
";

fn weapon() -> TestValue {
    TestValue::Struct(TestInstance::new("WeaponParams"))
}

fn star_map(builder: DataForgeBuilder) -> DataForgeBuilder {
    let object = |guid: u128, kind: &str, name: &str, target: u128| {
        TestRecord::new(
            guid,
            kind,
            format!("{kind}.{name}"),
            format!("libs/starmap/{name}.xml"),
        )
        .value("type", TestValue::Reference(target))
    };
    let kind = |guid: u128, name: &str| {
        TestRecord::new(
            guid,
            "StarMapObjectType",
            format!("StarMapObjectType.{name}"),
            format!("libs/starmap/type/{name}.xml"),
        )
        .value("name", TestValue::text(name))
    };
    builder
        .structure(TestStruct::new("StarMapObjectType").field("name", TestType::String))
        .structure(TestStruct::new("StarMapObject").field(
            "type",
            TestType::ReferenceTo("StarMapObjectType".to_owned()),
        ))
        .structure(TestStruct::new("DerivedStarMapObject").parent("StarMapObject"))
        .record(kind(20, "SolarSystem"))
        .record(kind(21, "Planet"))
        .record(object(19, "StarMapObject", "SolarSystem_Template", 20))
        .record(object(22, "StarMapObject", "stanton", 20))
        .record(object(23, "DerivedStarMapObject", "pyro", 20))
        .record(object(24, "StarMapObject", "hurston", 21))
}

fn database() -> Vec<u8> {
    let entity = |guid: u128, path: &str| {
        TestRecord::new(
            guid,
            "EntityClassDefinition",
            format!("Entity.E{guid}"),
            path,
        )
    };
    star_map(DataForgeBuilder::new())
        .structure(TestStruct::new("Params"))
        .structure(TestStruct::new("WeaponParams").parent("Params"))
        .structure(TestStruct::new("MissileParams").parent("Params"))
        .structure(TestStruct::new("HeavyWeaponParams").parent("WeaponParams"))
        .structure(TestStruct::new("Loop").parent("Loop"))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", TestType::StrongPointer("Params".to_owned())),
        )
        .structure(TestStruct::new("MissionItem"))
        .structure(
            TestStruct::new("Faction")
                .field("name", TestType::String)
                .field("rival", TestType::ReferenceTo("Faction".to_owned())),
        )
        .structure(TestStruct::new("SubFaction").parent("Faction"))
        .record(
            entity(1, "libs/entities/one.xml")
                .value("Components", TestValue::Array(vec![weapon(), weapon()])),
        )
        .record(
            entity(2, "libs/entities/two.xml")
                .value("Components", TestValue::Array(vec![weapon()])),
        )
        .record(entity(3, "libs/entities/three.xml").value(
            "Components",
            TestValue::Array(vec![TestValue::Struct(TestInstance::new(
                "HeavyWeaponParams",
            ))]),
        ))
        .record(TestRecord::new(
            12,
            "Loop",
            "Loop.Loop",
            "libs/loops/loop.xml",
        ))
        .record(
            TestRecord::new(10, "Faction", "Faction.Good", "libs/factions/good.xml")
                .value("name", TestValue::text("good")),
        )
        .record(
            TestRecord::new(11, "SubFaction", "Faction.Sub", "libs/factions/sub.xml")
                .value("rival", TestValue::Reference(0xdead)),
        )
        .build()
}

fn vehicle(name: &str) -> Vec<u8> {
    TestElement::new("Vehicle")
        .attribute("name", name)
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn archive(dir: &std::path::Path) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let mut broken = vehicle("Broken");
    broken.truncate(30);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()))
        .entry(TestEntry::zstd("Data\\Vehicles\\Good.xml", vehicle("Good")))
        .entry(TestEntry::zstd(
            "data/vehicles/Text.xml",
            b"<Vehicle/>".to_vec(),
        ))
        .entry(TestEntry::zstd("Data\\Vehicles\\Broken.xml", broken))
        .entry(TestEntry::zstd(
            "Data\\Vehicles\\Junk.xml",
            vec![0, 1, 2, 3, 0xff, 0xfe],
        ))
        .entry(TestEntry::zstd(
            "Data\\Other\\Elsewhere.xml",
            vehicle("Else"),
        ))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn report() -> Result<CoverageReport, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = archive(dir.path())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    Ok(CoverageReport::of(
        &archive,
        &forge,
        &ReferenceList::parse(LIST)?,
    ))
}

#[test]
fn record_types_are_counted_with_the_types_deriving_from_them() -> TestResult {
    let report = report()?;

    let [item, candidate, faction, looped] = report.types.as_slice() else {
        return Err(format!("four types: {:?}", report.types).into());
    };
    assert_eq!(
        (looped.records, looped.unreadable_records),
        (1, 1),
        "a self-parented struct ends the walk and counts as unreadable"
    );
    assert!(looped.sample.is_none());
    assert_eq!((item.defined, item.records), (true, 0));
    assert!(item.sample.is_none());
    assert_eq!((candidate.defined, candidate.candidate), (false, true));
    assert_eq!((faction.defined, faction.records), (true, 2));
    assert_eq!(faction.error_values, 1, "the dangling Reference");
    assert_eq!(faction.unreadable_records, 0);
    let sample = faction.sample.as_ref().ok_or("a sample")?;
    assert_eq!(sample.record_path, "libs/factions/good");
    assert!(sample.xml_bytes > 0 && sample.json_bytes > 0 && sample.error.is_none());
    Ok(())
}

#[test]
fn components_are_counted_in_the_entities_and_reached_by_a_query() -> TestResult {
    let report = report()?;

    let [weapon, missile, missing] = report.components.as_slice() else {
        return Err(format!("three components: {:?}", report.components).into());
    };
    assert_eq!(
        (weapon.entities, weapon.instances),
        (3, 4),
        "the derived HeavyWeaponParams counts"
    );
    assert_eq!(report.unreadable_entities, 0);
    assert!(weapon.defined && weapon.reachable, "{weapon:?}");
    assert_eq!((missile.entities, missile.instances), (0, 0));
    assert!(missile.defined && missile.reachable);
    assert!(!missing.defined && !missing.reachable);
    Ok(())
}

#[test]
fn starmap_objects_are_counted_by_the_name_of_their_type() -> TestResult {
    let report = report()?;

    let [system, nowhere] = report.objects.as_slice() else {
        return Err(format!("two starmap types: {:?}", report.objects).into());
    };
    assert_eq!(system.type_name, "SolarSystem");
    assert_eq!(
        (system.records, system.templates),
        (3, 1),
        "the derived struct and the template count, the Planet does not"
    );
    let sample = system.sample.as_ref().ok_or("a sample")?;
    assert_eq!(
        sample.record_path, "libs/starmap/stanton",
        "never the template"
    );
    assert!(sample.error.is_none());
    assert_eq!((nowhere.records, nowhere.sample.is_none()), (0, true));
    assert!(!report.is_complete());
    Ok(())
}

#[test]
fn entries_of_a_folder_are_converted_from_cryxml() -> TestResult {
    let report = report()?;

    let [folder] = report.entries.as_slice() else {
        return Err("one folder".into());
    };
    assert_eq!(folder.entries, 4);
    assert_eq!((folder.converted, folder.text), (1, 1));
    let [(broken, _), (junk, reason)] = folder.failures.as_slice() else {
        return Err(format!("two failures: {:?}", folder.failures).into());
    };
    assert_eq!(broken, "Data\\Vehicles\\Broken.xml");
    assert_eq!(junk, "Data\\Vehicles\\Junk.xml");
    assert_eq!(reason, "neither CryXml nor text XML");
    assert!(!report.is_complete());
    Ok(())
}

#[test]
fn the_built_in_list_is_valid_and_names_every_kind() -> TestResult {
    let list = ReferenceList::builtin()?;

    for kind in [
        ReferenceKind::Record,
        ReferenceKind::Candidate,
        ReferenceKind::Component,
        ReferenceKind::Entries,
        ReferenceKind::StarMapType,
    ] {
        assert!(
            list.items().iter().any(|item| item.kind == kind),
            "{kind:?}"
        );
    }
    assert_eq!(
        list.items()
            .iter()
            .filter(|item| item.kind == ReferenceKind::Record)
            .count(),
        36
    );
    assert_eq!(
        list.items()
            .iter()
            .filter(|item| item.kind == ReferenceKind::Component)
            .count(),
        20
    );
    Ok(())
}

#[test]
fn a_malformed_line_is_an_error() {
    assert!(ReferenceList::parse("missions record").is_err());
    assert!(ReferenceList::parse("missions thing Name").is_err());
    assert!(ReferenceList::parse("a record B extra").is_err());
}

#[path = "coverage_inheritance.rs"]
mod coverage_inheritance;
