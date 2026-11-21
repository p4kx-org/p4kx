// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::with_ship_sources;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{ShipBaseOptions, build_ship_base};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;

use super::fixture;

const DEFINITION: &str = "Scripts/Entities/Vehicles/Implementations/Xml/AggregateShip.xml";
const PATCH: &str =
    "Data/Scripts/Entities/Vehicles/Implementations/Xml/Modifications/rolling_fixture.xml";
const PATCHED: &[u8] = br#"<Vehicle><Parts>
    <Part id="chosen" name="chosen" class="Animated" mass="12"/>
    <Part name="later" class="Animated"/>
    </Parts><Modifications><Modification name="Rolling"
    patchFile="Modifications/rolling_fixture.xml"/></Modifications></Vehicle>"#;
const BASE: &[u8] = br#"<Vehicle><Parts>
    <Part name="kept" class="Animated" mass="12"/>
    <Part name="skipped" class="SubPartWheel" skipPart="true"/>
    <Part name="withoutClass"/>
    </Parts></Vehicle>"#;
const WHEEL_PATCH: &[u8] = br#"<Patch><Part id="chosen" class="SubPartWheel"/></Patch>"#;

#[test]
fn ground_path_classification_is_independent_of_encrypted_vehicle_xml() -> TestResult {
    let (_directory, archive) = open(
        "groundvehicles",
        TestEntry::zstd(fixture::XML, PATCHED).encrypted(),
        Some(WHEEL_PATCH),
    )?;
    let catalog = fixture::catalog(&archive)?;
    let row = super::one(&catalog)?;
    let kind = proof::present(row, "vehicleKind", &json!("groundVehicle"), Some(false))?;
    let sources = proof::uses(&catalog, kind)?;
    assert_eq!(
        sources.len(),
        2,
        "source roles of the admitted Record filename"
    );
    assert!(sources.iter().all(|used| {
        used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeRecordHeader")
            && used.pointer("/location/field").and_then(Value::as_str) == Some("fileName")
    }));
    verify_parity(&archive, &catalog, kind)
}

#[test]
fn nonrolling_decision_retains_used_part_class_and_membership() -> TestResult {
    let (_directory, archive) = open(
        "spaceships",
        TestEntry::zstd(fixture::XML, BASE).encrypted(),
        None,
    )?;
    let catalog = fixture::catalog(&archive)?;
    let kind = proof::present(
        super::one(&catalog)?,
        "vehicleKind",
        &json!("spaceship"),
        Some(true),
    )?;
    let class_paths = class_sources(&catalog, kind)?;
    assert_eq!(
        class_paths,
        [fixture::XML],
        "a skipped Part's class is not consulted"
    );
    let used = proof::uses(&catalog, kind)?;
    assert!(used.iter().any(|item| {
        item.pointer("/location/field").and_then(Value::as_str) == Some("$children")
    }));
    assert!(used.iter().any(|item| {
        item.pointer("/location/field").and_then(Value::as_str) == Some("$attributes")
    }));
    assert!(
        used.iter().all(|item| {
            item.pointer("/location/field").and_then(Value::as_str) != Some("mass")
        })
    );
    verify_parity(&archive, &catalog, kind)
}

#[test]
fn rolling_decision_uses_the_effective_patch_donor_instead_of_overwritten_classes() -> TestResult {
    let (_directory, archive) = open(
        "spaceships",
        TestEntry::zstd(fixture::XML, PATCHED),
        Some(WHEEL_PATCH),
    )?;
    let catalog = fixture::catalog(&archive)?;
    let kind = proof::cell(super::one(&catalog)?, "vehicleKind", "absent")?;
    assert_eq!(kind["containsEncryptedSource"], true);
    assert_eq!(kind["sourceSetComplete"], true);
    assert_eq!(class_sources(&catalog, kind)?, [PATCH]);
    let observed = proof::uses(&catalog, kind)?
        .into_iter()
        .find(|used| used["location"]["field"] == "class")
        .ok_or("the original wheel class selector")?;
    assert_eq!(observed["location"]["namespace"], "textXmlAttribute");
    let [start, end] = observed["location"]["indices"]
        .as_array()
        .ok_or("class byte coordinates")?
        .as_slice()
    else {
        return Err("one original class attribute range".into());
    };
    let offset = |coordinate: &Value| -> TestResult<usize> {
        Ok(coordinate
            .as_str()
            .ok_or("decimal byte coordinate")?
            .parse()?)
    };
    assert_eq!(
        WHEEL_PATCH.get(offset(start)?..offset(end)?),
        Some(br#"class="SubPartWheel""#.as_slice()),
        "the physical byte range contains the effective original donor",
    );
    verify_parity(&archive, &catalog, kind)
}

#[test]
fn tread_decision_retains_the_complete_no_wheel_survey() -> TestResult {
    let patch = br#"<Patch><Part id="chosen" class="Tread"/></Patch>"#;
    let (_directory, archive) = open(
        "spaceships",
        TestEntry::zstd(fixture::XML, PATCHED),
        Some(patch),
    )?;
    let catalog = fixture::catalog(&archive)?;
    let kind = proof::cell(super::one(&catalog)?, "vehicleKind", "absent")?;
    assert_eq!(kind["containsEncryptedSource"], true);
    assert_eq!(kind["sourceSetComplete"], true);
    let mut paths = class_sources(&catalog, kind)?;
    paths.sort_unstable();
    assert_eq!(paths, [fixture::XML, PATCH]);
    verify_parity(&archive, &catalog, kind)
}

#[test]
fn failed_encrypted_patch_keeps_the_legacy_kind_with_incomplete_evidence() -> TestResult {
    let (_directory, archive) = open(
        "spaceships",
        TestEntry::zstd(fixture::XML, PATCHED),
        Some(b"<broken>"),
    )?;
    let catalog = fixture::catalog(&archive)?;
    let kind = proof::cell(super::one(&catalog)?, "vehicleKind", "present")?;
    assert_eq!(
        kind.get("value").and_then(Value::as_str),
        Some("spaceship"),
        "the fallback keeps the standalone classification"
    );
    assert_eq!(
        kind.get("containsEncryptedSource"),
        Some(&Value::Null),
        "a failed encrypted patch is not a contributing source"
    );
    assert_eq!(
        kind.get("sourceSetComplete"),
        Some(&Value::Bool(false)),
        "the missing override prevents complete field provenance"
    );
    assert_eq!(class_sources(&catalog, kind)?, [fixture::XML, fixture::XML]);
    verify_parity(&archive, &catalog, kind)
}

fn open(
    folder_kind: &str,
    vehicle: TestEntry,
    patch: Option<&'static [u8]>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let archive_file = directory.path().join("Data.p4k");
    let owner = TestEntity::new(
        0x7511,
        "AggregateShip",
        format!("libs/foundry/records/entities/{folder_kind}/aggregate.xml"),
    )
    .attach_type("NOITEM_Vehicle")
    .vehicle(DEFINITION, if patch.is_some() { "Rolling" } else { "" });
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .record(owner.record())
        .build();
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", database))
        .entry(vehicle);
    if let Some(patch) = patch {
        archive = archive.entry(TestEntry::zstd(PATCH, patch).encrypted());
    }
    archive.write_to(&archive_file)?;
    Ok((
        directory,
        Archive::open_with_key(&archive_file, Some(p4kx_testkit::test_key()))?,
    ))
}

fn class_sources<'catalog>(
    catalog: &'catalog Value,
    kind: &Value,
) -> TestResult<Vec<&'catalog str>> {
    let mut sources = Vec::new();
    for used in proof::uses(catalog, kind)? {
        if used
            .pointer("/location/field")
            .and_then(Value::as_str)
            .is_some_and(|field| field.trim_start_matches('@') == "class")
        {
            assert_eq!(
                used["role"], "selector",
                "a class is used to select the vehicle kind"
            );
            let source = proof::source(catalog, used)?;
            sources.push(
                source
                    .pointer("/path/value")
                    .and_then(Value::as_str)
                    .ok_or("physical class source Entry")?,
            );
        }
    }
    Ok(sources)
}

fn verify_parity(archive: &Archive, catalog: &Value, kind: &Value) -> TestResult {
    let built = build_ship_base(archive, &ShipBaseOptions::new(UNIX_EPOCH))?;
    let [base] = built.ships() else {
        return Err("one standalone ship".into());
    };
    assert_eq!(
        kind["value"],
        json!(base.vehicle_kind),
        "the Catalog keeps the standalone vehicle classification"
    );
    with_ship_sources(
        archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one selected SDK Ship".into());
            };
            let context = json!({"provenance": snapshot.source_inventory()});
            assert_eq!(
                proof::with_origins(serde_json::to_value(row.vehicle_kind())?, &context)?,
                proof::with_origins(kind.clone(), catalog)?,
                "the SDK and Catalog retain the same vehicle kind and source uses",
            );
            Ok(())
        },
    )??;
    proof::closure(catalog)
}
