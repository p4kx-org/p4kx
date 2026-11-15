// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Fixture, TestResult, check_coverage, replace_value};

const OWNER: &str = "EntityClassDefinition";
const BASE: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ATTACH_DEFINITION: &str = "SItemDefinition";
const LOCALIZATION: &str = "SCItemLocalization";
const PURCHASE: &str = "SCItemPurchasableParams";
const COMPONENTS: &str = "/definitions/items/rows/0/components/value";

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

    for (path, domain, form, expected) in [
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
    ] {
        let catalog = mutate_german(&fixture, path, domain, form)?;
        let (english, german) = fixture.coverage(&catalog, Kind::Items)?;
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
    let (english, german) = fixture.coverage(&catalog, Kind::Items)?;
    assert_eq!(
        english.total(),
        0,
        "later slots cannot change the presentation selection"
    );
    assert_eq!(german.total(), 0);

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
            purchasable("@own_type"),
            attachment("@resource_name", "@own_description"),
            purchasable("@fallback_resource_name"),
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

fn purchasable(key: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(PURCHASE).value("displayType", TestValue::text(key)))
}
