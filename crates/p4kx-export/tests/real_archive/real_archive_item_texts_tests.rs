// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Coverage, Fixture, TestResult, check_coverage, replace_value};
use crate::catalog_v3_consumer;
use crate::real_archive_texts::live::census::read_fixture_through_pipe;

const OWNER: &str = "EntityClassDefinition";
const BASE: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ATTACH_DEFINITION: &str = "SItemDefinition";
const LOCALIZATION: &str = "SCItemLocalization";
const PURCHASE: &str = "SCItemPurchasableParams";
const COMPONENTS: &str = "/definitions/items/rows/0/components/value";

const MUTATIONS: [(&str, &str, &str, (usize, usize)); 6] = [
    (
        "0/value/value/localization/value/name/value",
        "publicNames",
        "displayText",
        (1, 0),
    ),
    (
        "0/value/value/localization/value/description/value",
        "gameTexts",
        "sourceText",
        (1, 0),
    ),
    (
        "1/value/displayType/value",
        "gameTexts",
        "displayText",
        (1, 0),
    ),
    (
        "1/value/displayName/value",
        "publicNames",
        "displayText",
        (0, 1),
    ),
    (
        "2/value/value/localization/value/name/value",
        "publicNames",
        "sourceText",
        (0, 1),
    ),
    (
        "3/value/displayType/value",
        "gameTexts",
        "displayText",
        (0, 1),
    ),
];

#[test]
fn item_translation_mutations_fail_without_changing_historical_baselines() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let (english, german) = fixture.coverage(&fixture.catalog, Kind::Items)?;
    assert_eq!(
        english.total(),
        3,
        "the original selections enter the English baseline"
    );
    assert_eq!(german.resolved, 3, "the original German texts resolve");
    assert_eq!(german.supplemental_contradictions, 0);

    for (path, domain, form, expected) in MUTATIONS {
        let catalog = mutate_german(&fixture, path, domain, form)?;
        let (english, german) = check_pipe_coverage(&fixture, &catalog, path)?;
        assert_eq!(english.total(), 3, "English denominator is preserved");
        assert_eq!(german.total(), 3, "German denominator is preserved");
        assert_eq!(
            (german.contradictions, german.supplemental_contradictions),
            expected
        );
        let mut failures = Vec::new();
        check_coverage(Kind::Items, &english, &german, true, 1, &mut failures);
        assert_eq!(
            failures.len(),
            1,
            "the contradiction independently fails the kind"
        );
    }
    Ok(())
}

#[test]
fn the_first_item_slot_controls_presentation_even_if_its_text_is_absent() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let mut catalog = fixture.catalog.clone();
    for path in ["0/value/value/localization", "1/value/displayType"] {
        replace_value(
            &mut catalog,
            &format!("{COMPONENTS}/{path}/state"),
            json!("absent"),
        )?;
        replace_value(
            &mut catalog,
            &format!("{COMPONENTS}/{path}/value"),
            Value::Null,
        )?;
    }
    let (english, german) = check_pipe_coverage(&fixture, &catalog, "absent first-slot text")?;
    assert_eq!(
        english.total(),
        0,
        "later slots cannot change the presentation selection"
    );
    assert_eq!(german.total(), 0);
    let mut failures = Vec::new();
    check_coverage(Kind::Items, &english, &german, false, 1, &mut failures);
    assert!(
        failures.is_empty(),
        "a selected Item may lack first-slot texts"
    );
    check_coverage(Kind::Items, &english, &german, true, 1, &mut failures);
    assert_eq!(
        failures.len(),
        1,
        "a positive LIVE baseline must retain keyed texts"
    );

    let record = catalog
        .pointer_mut("/items/0/recordIndex/value")
        .ok_or("physical Item position")?;
    *record = json!(1);
    assert!(
        fixture.coverage(&catalog, Kind::Items).is_err(),
        "the physical owner cannot be guessed"
    );
    Ok(())
}

#[test]
fn item_text_census_keeps_positions_and_checked_carriers_without_the_nontext_body() -> TestResult {
    const MANUFACTURER: &str =
        "/definitions/items/rows/0/components/value/0/value/value/manufacturer";
    let fixture = Fixture::from_database(&database())?;
    let mut full = fixture.catalog.clone();
    replace_value(
        &mut full,
        MANUFACTURER,
        json!({"value": ["opaque manufacturer data".repeat(16_384)]}),
    )?;
    let projected = read_fixture_through_pipe(&serde_json::to_vec(&full)?)?;
    assert!(projected.pointer(MANUFACTURER).is_none());
    for path in [
        "/definitions/items/rows/0/recordIndex",
        "/definitions/items/rows/0/components/sourceUsesRefs",
        "/definitions/items/rows/0/components/value/1/value/displayName",
        "/definitions/items/rows/0/components/value/2/componentOrdinal",
        "/definitions/items/rows/0/components/value/2/value/value/localization/value/description",
    ] {
        let expected = full.pointer(path).ok_or("original text-census data")?;
        assert_eq!(projected.pointer(path), Some(expected), "retained {path}");
    }
    assert_eq!(
        catalog_v3_consumer::rows(&projected, Kind::Items)?,
        catalog_v3_consumer::rows(&full, Kind::Items)?,
        "flat Item rows and their counts remain intact"
    );
    assert_eq!(
        fixture.coverage(&projected, Kind::Items)?,
        fixture.coverage(&full, Kind::Items)?
    );
    check_corrupt_item_carriers(&fixture)?;
    assert!(
        read_fixture_through_pipe(
            br#"{"definitions":{"items":{"rows":[{"components":{"value":[{"value":{"value":{"manufacturer":[oops]}}}]}}]}}}"#,
        )
        .is_err(),
        "ignored Item payload still needs valid JSON"
    );
    Ok(())
}

fn check_pipe_coverage(
    fixture: &Fixture,
    catalog: &Value,
    context: &str,
) -> TestResult<(Coverage, Coverage)> {
    let expected = fixture.coverage(catalog, Kind::Items)?;
    let projected = read_fixture_through_pipe(&serde_json::to_vec(catalog)?)?;
    assert_eq!(
        fixture.coverage(&projected, Kind::Items)?,
        expected,
        "same Item text coverage for {context}"
    );
    Ok(expected)
}

fn check_corrupt_item_carriers(fixture: &Fixture) -> TestResult {
    for (path, replacement) in [
        ("/definitions/items/rows/0/recordIndex/value", json!(7)),
        (
            "/definitions/items/rows/0/components/value/0/value/containsEncryptedSource",
            json!("invalid"),
        ),
        (
            "/definitions/items/rows/0/components/value/2/kind",
            json!("unknown"),
        ),
        (COMPONENTS, json!("not an array")),
    ] {
        let mut corrupted = fixture.catalog.clone();
        replace_value(&mut corrupted, path, replacement)?;
        let projected = read_fixture_through_pipe(&serde_json::to_vec(&corrupted)?)?;
        let census = |document| {
            fixture
                .coverage(document, Kind::Items)
                .map_err(|error| error.to_string())
        };
        let expected = census(&corrupted);
        assert!(expected.is_err(), "bad original {path} fails the oracle");
        assert_eq!(census(&projected), expected, "same failure for {path}");
    }
    Ok(())
}

fn mutate_german(fixture: &Fixture, path: &str, domain: &str, form: &str) -> TestResult<Value> {
    let row = "/definitions/items/rows/0/components/value";
    let id = fixture
        .catalog
        .pointer(&format!("{row}/{path}"))
        .ok_or("actual canonical Item text reference")?;
    let mut catalog = fixture.catalog.clone();
    let translations = catalog
        .pointer_mut(&format!("/texts/{domain}"))
        .and_then(Value::as_array_mut)
        .ok_or("canonical Item text table")?;
    let translation = translations
        .iter_mut()
        .find(|translation| translation.pointer("/id/value") == Some(id))
        .ok_or("canonical Item text target")?;
    replace_value(
        translation,
        &format!("/translations/value/de/{form}/value"),
        json!("Falscher Text"),
    )?;
    Ok(catalog)
}

fn database() -> DataForgeBuilder {
    let item = TestRecord::new(
        1,
        OWNER,
        "EntityClassDefinition.ItemPresentation",
        "libs/foundry/records/entities/scitem/fixture/item_presentation.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            attachment("@shared", "@resource_description"),
            purchasable("@fallback_resource_name", "@own_type"),
            attachment("@resource_name", "@own_description"),
            purchasable("@own_description", "@fallback_resource_name"),
        ]),
    );

    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(OWNER).array("Components", TestType::StrongPointer(BASE.into())))
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(BASE)
                .field("AttachDef", TestType::Class(ATTACH_DEFINITION.into())),
        )
        .structure(
            TestStruct::new(ATTACH_DEFINITION)
                .field("Localization", TestType::Class(LOCALIZATION.into())),
        )
        .structure(
            TestStruct::new(LOCALIZATION)
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new(PURCHASE)
                .parent(BASE)
                .field("displayName", TestType::Locale)
                .field("displayType", TestType::Locale),
        )
        .record(item)
}

fn attachment(name: &str, description: &str) -> TestValue {
    let localization = TestInstance::new(LOCALIZATION)
        .value("Name", TestValue::text(name))
        .value("Description", TestValue::text(description));
    let definition =
        TestInstance::new(ATTACH_DEFINITION).value("Localization", TestValue::Struct(localization));
    TestValue::Struct(
        TestInstance::new(ATTACHMENT).value("AttachDef", TestValue::Struct(definition)),
    )
}

fn purchasable(name: &str, display_type: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PURCHASE)
            .value("displayName", TestValue::text(name))
            .value("displayType", TestValue::text(display_type)),
    )
}

#[test]
fn source_backed_item_headers_match_the_original_definition_in_text_consumers() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    fixture.coverage(&fixture.catalog, Kind::Items)?;
    for field in ["guid", "recordPath", "recordIndex", "sourceType"] {
        let mut document = fixture.catalog.clone();
        let path = format!("/items/0/{field}");
        let payload = catalog_v3_consumer::present(
            document
                .pointer(&path)
                .ok_or("actual producer source header")?,
        )?
        .clone();
        replace_value(&mut document, &path, payload)?;
        assert!(
            fixture.coverage(&document, Kind::Items).is_err(),
            "an unsourced {field} cannot impersonate the physical definition"
        );
    }
    let mut document = fixture.catalog.clone();
    replace_value(
        &mut document,
        "/items/0/guid/containsEncryptedSource",
        json!(true),
    )?;
    assert!(
        fixture.coverage(&document, Kind::Items).is_err(),
        "identical nominal payload does not replace exact original source cells"
    );
    Ok(())
}

#[path = "real_archive_item_source_selection_tests.rs"]
mod source_selection;
