// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Guid, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::record_link::RecordLink;
use super::selected_texts::SelectedTexts;
use super::starmap_definitions::{self, AMENITIES, DefinitionFamily, TYPES};
use crate::build::original_input::test_support::TestResult;
use crate::catalog_queries::starmap::{
    StarmapDefinitionFamily, StarmapDefinitionRows, StarmapDefinitionSelection,
};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

const TYPE: &str = "StarMapObjectType";
const AMENITY: &str = "StarMapAmenityTypeEntry";
const OBJECT: &str = "StarMapObject";
const OBJECTS: &str = "definitions.starmapObjects";
const TYPE_GUID: u128 = 0x1177_9100;
const ALIAS_GUID: u128 = 0x1177_9101;
const AMENITY_GUID: u128 = 0x1177_9102;
const OBJECT_GUID: u128 = 0x1177_9103;

#[derive(Clone, Copy)]
enum ObstaclePolicy {
    Authored,
    Undeclared,
}

#[test]
fn reduced_starmap_definition_families_close_original_links_and_locales() -> TestResult {
    with_queries(ObstaclePolicy::Authored, |parsed, scratch| {
        let mut texts = SelectedTexts::default();
        let types = starmap_definitions::collect_types(parsed, scratch, None, &mut texts)?;
        let amenities = starmap_definitions::collect_amenities(parsed, scratch, None)?;
        verify_object_links(parsed, &types, &amenities)?;

        let table = serde_json::to_value(types.table)?;
        let [first, alias, second_alias] = rows(&table)? else {
            return Err("three physical Starmap types".into());
        };
        verify_type(first)?;
        let alias_guid = row_guid(alias)?;
        assert_eq!(row_guid(second_alias)?, alias_guid, "equal original GUIDs");
        for (row, index) in [(alias, 1), (second_alias, 3)] {
            assert_eq!(row.pointer("/recordIndex/value"), Some(&json!(index)));
            types.targets.verify(TYPES, index, alias_guid)?;
        }
        verify_texts(first, &serde_json::to_value(texts)?)?;

        let table = serde_json::to_value(amenities.table)?;
        let [amenity] = rows(&table)? else {
            return Err("one admitted Amenity definition".into());
        };
        assert_eq!(
            amenity.pointer("/name/value"),
            Some(&json!("FixtureMedical"))
        );
        assert_eq!(
            amenity.pointer("/name/containsEncryptedSource"),
            Some(&json!(true))
        );
        assert_eq!(amenity.pointer("/recordIndex/value"), Some(&json!(2)));
        amenities.targets.verify(AMENITIES, 2, row_guid(amenity)?)?;
        assert!(
            amenity.get("displayName").is_none(),
            "the selected Amenity uses its authored name"
        );
        assert!(
            amenity.get("icon").is_none(),
            "the selected Amenity has no media field"
        );
        Ok(())
    })
}

#[test]
fn undeclared_starmap_obstacle_policy_stays_distinct_from_false() -> TestResult {
    with_queries(ObstaclePolicy::Undeclared, |parsed, scratch| {
        let types = starmap_definitions::collect_types(
            parsed,
            scratch,
            None,
            &mut SelectedTexts::default(),
        )?;
        let table = serde_json::to_value(types.table)?;
        for row in rows(&table)? {
            assert_eq!(
                row.pointer("/ignoreAsRoutingObstacle/state"),
                Some(&json!("absent")),
                "an undeclared rule leaves the value absent"
            );
            assert_eq!(
                row.pointer("/ignoreAsRoutingObstacle/value"),
                Some(&Value::Null)
            );
        }
        Ok(())
    })
}

fn verify_type(row: &Value) -> TestResult {
    assert_eq!(row.pointer("/name/value"), Some(&json!("Fixture_ValidQT")));
    for (name, value) in [
        ("validQuantumTravelDestination", false),
        ("ignoreAsRoutingObstacle", true),
    ] {
        let policy = row.get(name).ok_or("selected typed policy")?;
        assert_eq!(
            policy.get("state"),
            Some(&json!("present")),
            "original {name} state"
        );
        assert_eq!(
            policy.get("value"),
            Some(&json!(value)),
            "original {name}, independent of name suffix"
        );
        assert_eq!(
            policy.get("containsEncryptedSource"),
            Some(&json!(true)),
            "original {name} marker"
        );
    }
    assert!(
        row.get("authoredFields").is_none(),
        "the original arena is outside the selected payload"
    );
    assert!(
        row.get("selectable").is_none(),
        "unselected navigation flags stay outside the table"
    );
    Ok(())
}

fn verify_texts(row: &Value, texts: &Value) -> TestResult {
    let key = row
        .pointer("/classification/value")
        .ok_or("selected classification reference")?;
    let Some([label]) = texts
        .get("publicNames")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Err("one referenced classification key".into());
    };
    assert_eq!(
        label.pointer("/id/value"),
        Some(key),
        "exact public classification key"
    );
    for (language, value) in [("en", "Station"), ("de", "Raumstation")] {
        assert_eq!(
            label.pointer(&format!("/translations/value/{language}/displayText/value")),
            Some(&json!(value)),
            "translation for {language}"
        );
    }
    assert_eq!(
        texts.get("gameTexts"),
        Some(&json!([])),
        "only the selected public name is referenced"
    );
    Ok(())
}

fn verify_object_links(
    parsed: &ParsedCatalogQueries<'_>,
    types: &DefinitionFamily,
    amenities: &DefinitionFamily,
) -> TestResult {
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Objects,
        CatalogRecordSelection::All,
    );
    parsed.with_starmap_definitions(selection, None, |snapshot| -> TestResult {
        let StarmapDefinitionRows::Objects(rows) = &snapshot.rows else {
            return Err("canonical objects".into());
        };
        let [object] = rows.as_slice() else {
            return Err("one known authored object with both definitions".into());
        };
        let kind = object.links.r#type.value().ok_or("typed link")?;
        let [amenity] = object
            .links
            .amenities
            .value()
            .ok_or("amenity slots")?
            .as_slice()
        else {
            return Err("one authored amenity slot".into());
        };
        let amenity = amenity.value().ok_or("amenity link")?;
        for (original, targets, expected_index) in
            [(kind, &types.targets, 0), (amenity, &amenities.targets, 2)]
        {
            let resolved = RecordLink::read_source(OBJECTS, original, targets, |_| true)?;
            let resolved = serde_json::to_value(resolved)?;
            assert_eq!(
                resolved.get("status"),
                Some(&json!("present")),
                "typed original link resolution"
            );
            assert_eq!(
                resolved.pointer("/targetRecordIndex/value"),
                Some(&json!(expected_index)),
                "exact staged physical target"
            );
        }
        assert!(
            matches!(
                RecordLink::read_source(OBJECTS, kind, &amenities.targets, |_| true),
                Err(ReducedError::UnresolvedRecordLink {
                    path: OBJECTS,
                    index: 0
                })
            ),
            "the two physical target inventories remain independent"
        );
        Ok(())
    })?
}

fn row_guid(row: &Value) -> TestResult<Guid> {
    Ok(row
        .pointer("/guid/value")
        .and_then(Value::as_str)
        .ok_or("original GUID")?
        .parse()?)
}

fn rows(table: &Value) -> TestResult<&[Value]> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "staged definition rows".into())
}

fn with_queries(
    obstacle: ObstaclePolicy,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let locale = |classification| {
        GlobalIniBuilder::new()
            .text("fixture_classification", classification)
            .text("fixture_amenity_label", "Unselected")
            .build()
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database(obstacle).build()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            locale("Station"),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            locale("Raumstation"),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    inspect(&session.parsed(None)?, scratch.path())
}

fn database(obstacle: ObstaclePolicy) -> DataForgeBuilder {
    let mut kind = TestStruct::new(TYPE)
        .field("name", TestType::String)
        .field("classification", TestType::Locale)
        .field("validQuantumTravelDestination", TestType::Bool)
        .field("selectable", TestType::Bool);
    if matches!(obstacle, ObstaclePolicy::Authored) {
        kind = kind.field("ignoreAsRoutingObstacle", TestType::Bool);
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(kind)
        .structure(
            TestStruct::new(AMENITY)
                .field("name", TestType::String)
                .field("displayName", TestType::Locale)
                .field("icon", TestType::String),
        )
        .structure(
            TestStruct::new(OBJECT)
                .field("type", TestType::ReferenceTo(TYPE.to_owned()))
                .array("amenities", TestType::ReferenceTo(AMENITY.to_owned())),
        )
        .record(type_record(TYPE_GUID, "Fixture_ValidQT", obstacle))
        .record(type_record(ALIAS_GUID, "AliasOne", obstacle))
        .record(
            TestRecord::new(
                AMENITY_GUID,
                AMENITY,
                "StarMapAmenityTypeEntry.FixtureMedical",
                "fixtures/medical.xml",
            )
            .value("name", TestValue::text("FixtureMedical"))
            .value("displayName", TestValue::text("@fixture_amenity_label"))
            .value("icon", TestValue::text("fixtures/icon.dds")),
        )
        .record(type_record(ALIAS_GUID, "AliasTwo", obstacle))
        .record(
            TestRecord::new(
                OBJECT_GUID,
                OBJECT,
                "StarMapObject.Fixture",
                "libs/foundry/records/starmap/pu/system/fixture.xml",
            )
            .value("type", TestValue::Reference(TYPE_GUID))
            .value(
                "amenities",
                TestValue::Array(vec![TestValue::Reference(AMENITY_GUID)]),
            ),
        )
}

fn type_record(id: u128, name: &str, obstacle: ObstaclePolicy) -> TestRecord {
    let record = TestRecord::new(
        id,
        TYPE,
        format!("{TYPE}.{name}"),
        format!("fixtures/{name}.xml"),
    )
    .value("name", TestValue::text(name))
    .value("classification", TestValue::text("@fixture_classification"))
    .value("validQuantumTravelDestination", TestValue::Bool(false))
    .value("selectable", TestValue::Bool(true));
    match obstacle {
        ObstaclePolicy::Authored => record.value("ignoreAsRoutingObstacle", TestValue::Bool(true)),
        ObstaclePolicy::Undeclared => record,
    }
}
