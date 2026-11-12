// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as archive, DcbOrigin, TestResult, at, guid, text_row};
use crate::health_origin_assertions as proof;

const OBJECT: u128 = 0x0010_6711;
const JURISDICTION: u128 = 0x0010_6712;
const LEGACY: u128 = 0x0010_6713;
const FACTION: u128 = 0x0010_6714;
const AMENITY: u128 = 0x0010_6715;
const MISSING: u128 = 0x0010_6716;
const TAG: u128 = 0x0010_6717;
const OBJECT_TYPE: &str = "StarMapObject";
const JURISDICTION_TYPE: &str = "Jurisdiction";
const LEGACY_TYPE: &str = "Faction_LEGACY";
const AMENITY_TYPE: &str = "StarMapAmenityTypeEntry";
const LOCATION_TYPE: &str = "StarMapObjectLocationParams";
const OVERRIDE_TYPE: &str = "StarmapBoolOverride";

#[test]
fn starmap_objects_keep_original_flags_jurisdiction_legacy_affiliation_and_amenities() -> TestResult
{
    let (_folder, input) = archive::open(&database(), DcbOrigin::Encrypted)?;
    let catalog = document(&input, None)?;
    let object = object(&catalog)?;
    verify_flags(&catalog, object)?;
    verify_links(object)?;
    assert_eq!(
        at(&catalog, "/definitions/jurisdictions/rows/0/guid/value")?,
        &json!(guid(JURISDICTION)),
        "the existing physical Jurisdiction definition"
    );
    assert_eq!(
        at(&catalog, "/definitions/factions/rows/0/guid/value")?,
        &json!(guid(FACTION)),
        "separate modern Faction identity"
    );
    assert_eq!(
        at(&catalog, "/definitions/tags/rows/0/guid/value")?,
        &json!(guid(TAG)),
        "the direct location hierarchy tag resolves in the existing taxonomy"
    );
    let placement = at(&catalog, "/placements/starmapPlacements/rows/0")?;
    proof::present(placement, "objectGuid", &json!(guid(OBJECT)), Some(true))?;
    proof::cell(placement, "position", "absent")?;
    assert!(
        object.get("position").is_none(),
        "computed position has its own table"
    );
    assert!(
        catalog.get("starmapObjects").is_none(),
        "the canonical table owns the definition"
    );
    proof::closure(&catalog)
}

#[test]
fn starmap_objects_keep_filtered_targets_as_source_links_and_filter_health() -> TestResult {
    let (_folder, input) = archive::open(&database(), DcbOrigin::Plain)?;
    let filter = Filter::new("*/starmap/pu/*");
    let catalog = document(&input, Some(&filter))?;
    let object = object(&catalog)?;
    verify_links(object)?;
    assert!(
        proof::rows(&catalog, "/definitions/jurisdictions/rows")?.is_empty(),
        "canonical jurisdiction selection follows the filter"
    );
    assert!(
        proof::rows(&catalog, "/definitions/starmapAmenities/rows")?.is_empty(),
        "canonical amenity selection follows the filter"
    );
    let links = proof::rows(&catalog, "/exportHealth/unresolvedReferences")?;
    for guid_value in [JURISDICTION, AMENITY] {
        let expected = json!(guid(guid_value));
        assert!(
            links.iter().any(|link| {
                link.pointer("/targetGuid/value") == Some(&expected)
                    && link["kind"] == "starmapObjects"
                    && link["status"] == "excludedByFilter"
            }),
            "original link records its separate filter result"
        );
    }
    proof::closure(&catalog)
}

fn verify_flags(catalog: &Value, object: &Value) -> TestResult<()> {
    proof::present(object, "guid", &json!(guid(OBJECT)), Some(true))?;
    proof::present(object, "hideInStarmap", &json!(true), Some(true))?;
    proof::present(object, "hideInWorld", &json!(false), Some(true))?;
    proof::present(object, "blockTravel", &json!(true), Some(true))?;
    proof::present(
        object,
        "overrideSpawnNavPoints",
        &json!("FixtureFutureOverride"),
        Some(true),
    )?;
    proof::cell(object, "isScannable", "absent")?;
    proof::present(object, "noAutoBodyRecovery", &json!(false), Some(true))?;
    assert!(
        object.get("released").is_none(),
        "no derived release policy"
    );
    let location = at(object, "/locationParams/value")?;
    proof::present(location, "excludeFromLevelLoad", &json!(false), Some(true))?;
    proof::present(
        location,
        "exposeForPlayerCreatedMissions",
        &json!(true),
        Some(true),
    )?;
    let name = text_row(catalog, "gameTexts", at(object, "/name/value")?)?;
    assert_eq!(
        at(name, "/key/value")?,
        "@shared",
        "original protected name"
    );
    Ok(())
}

fn verify_links(object: &Value) -> TestResult<()> {
    for (field, guid_value, target_type) in [
        ("jurisdiction", JURISDICTION, JURISDICTION_TYPE),
        ("affiliation", LEGACY, LEGACY_TYPE),
        ("locationHierarchyTag", TAG, "Tag"),
    ] {
        let link = at(object, &format!("/{field}/value"))?;
        assert_eq!(
            at(link, "/guid/value")?,
            &json!(guid(guid_value)),
            "authored target"
        );
        assert_eq!(
            at(link, "/targetType/value")?,
            target_type,
            "exact physical type"
        );
        assert_eq!(at(link, "/status")?, "present", "original resolved target");
    }
    let links = proof::rows(object, "/amenities/value")?;
    assert_eq!(
        links.len(),
        3,
        "ordered occurrences include the repeated amenity"
    );
    for (link, (expected, state)) in links.iter().zip([
        (AMENITY, "present"),
        (MISSING, "dangling"),
        (AMENITY, "present"),
    ]) {
        assert_eq!(
            at(link, "/value/guid/value")?,
            &json!(guid(expected)),
            "exact authored GUID"
        );
        assert_eq!(
            at(link, "/value/status")?,
            state,
            "actual original link availability"
        );
    }
    Ok(())
}

fn object(catalog: &Value) -> TestResult<&Value> {
    at(catalog, "/definitions/starmapObjects/rows/0")
}

fn document(input: &p4kx_extraction::Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = archive::options();
    options.filter = filter;
    Ok(serde_json::from_slice(
        build_catalog(input, &options)?.json(),
    )?)
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(OVERRIDE_TYPE, ["FixtureFutureOverride"])
        .structure(object_declaration())
        .structure(TestStruct::new(JURISDICTION_TYPE).field("baseFine", TestType::Int32))
        .structure(TestStruct::new(LEGACY_TYPE).field("displayName", TestType::Locale))
        .structure(TestStruct::new("Faction").field("name", TestType::Locale))
        .structure(TestStruct::new(AMENITY_TYPE).field("displayName", TestType::Locale))
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .structure(
            TestStruct::new(LOCATION_TYPE)
                .field("excludeFromLevelLoad", TestType::Bool)
                .field("exposeForPlayerCreatedMissions", TestType::Bool),
        )
        .record(object_record())
        .record(other(JURISDICTION, JURISDICTION_TYPE).value("baseFine", TestValue::Int32(0)))
        .record(other(LEGACY, LEGACY_TYPE).value("displayName", TestValue::text("@shared")))
        .record(other(FACTION, "Faction").value("name", TestValue::text("@shared")))
        .record(other(AMENITY, AMENITY_TYPE).value("displayName", TestValue::text("@shared")))
        .record(other(TAG, "Tag").value("tagName", TestValue::text("Fixture.Navigation")))
}

fn object_declaration() -> TestStruct {
    TestStruct::new(OBJECT_TYPE)
        .field("name", TestType::Locale)
        .field("affiliation", reference(LEGACY_TYPE))
        .field("jurisdiction", reference(JURISDICTION_TYPE))
        .field("locationHierarchyTag", reference("Tag"))
        .field("hideInStarmap", TestType::Bool)
        .field("hideInWorld", TestType::Bool)
        .field("blockTravel", TestType::Bool)
        .field("noAutoBodyRecovery", TestType::Bool)
        .field(
            "overrideSpawnNavPoints",
            TestType::Enum(OVERRIDE_TYPE.to_owned()),
        )
        .field(
            "locationParams",
            TestType::StrongPointer(LOCATION_TYPE.to_owned()),
        )
        .array("amenities", reference(AMENITY_TYPE))
}

fn object_record() -> TestRecord {
    TestRecord::new(
        OBJECT,
        OBJECT_TYPE,
        "StarMapObject.Fixture",
        "libs/foundry/records/starmap/pu/fixture.xml",
    )
    .value("name", TestValue::text("@shared"))
    .value("affiliation", TestValue::Reference(LEGACY))
    .value("jurisdiction", TestValue::Reference(JURISDICTION))
    .value("locationHierarchyTag", TestValue::Reference(TAG))
    .value("hideInStarmap", TestValue::Bool(true))
    .value("hideInWorld", TestValue::Bool(false))
    .value("blockTravel", TestValue::Bool(true))
    .value("noAutoBodyRecovery", TestValue::Bool(false))
    .value(
        "overrideSpawnNavPoints",
        TestValue::text("FixtureFutureOverride"),
    )
    .value(
        "locationParams",
        TestValue::Struct(
            TestInstance::new(LOCATION_TYPE)
                .value("excludeFromLevelLoad", TestValue::Bool(false))
                .value("exposeForPlayerCreatedMissions", TestValue::Bool(true)),
        ),
    )
    .value(
        "amenities",
        TestValue::Array(
            [AMENITY, MISSING, AMENITY]
                .into_iter()
                .map(TestValue::Reference)
                .collect(),
        ),
    )
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn other(guid: u128, kind: &str) -> TestRecord {
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.Fixture"),
        format!("outside/{guid:x}.xml"),
    )
}
