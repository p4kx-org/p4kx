// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{Coverage, Fixture, TestResult, check_coverage, replace_value};
use crate::real_archive_canonical_texts;

const OWNER: &str = "EntityClassDefinition";
const BASE: &str = "DataForgeComponentParams";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ATTACH_DEFINITION: &str = "SItemDefinition";
const LOCALIZATION: &str = "SCItemLocalization";
const PURCHASE: &str = "SCItemPurchasableParams";

#[test]
fn item_translation_mutations_fail_without_changing_historical_baselines() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let (english, german) = coverage(&fixture, &fixture.catalog)?;
    assert_eq!(
        english.total(),
        3,
        "legacy English baseline selects the original fields"
    );
    assert_eq!(german.resolved, 3, "the original German texts resolve");
    assert_eq!(
        german.supplemental_contradictions, 0,
        "canonical Item texts agree with the INI"
    );

    for (path, domain, form) in [
        (
            "0/value/value/localization/value/name/value",
            "publicNames",
            "displayText",
        ),
        (
            "0/value/value/localization/value/description/value",
            "gameTexts",
            "sourceText",
        ),
        ("1/value/displayType/value", "gameTexts", "displayText"),
    ] {
        let catalog = mutate_german(&fixture, path, domain, form)?;
        let (english, german) = coverage(&fixture, &catalog)?;
        assert_eq!(english.total(), 3, "English denominator is preserved");
        assert_eq!(german.total(), 3, "German denominator is preserved");
        assert_eq!(
            german.resolved, 3,
            "original legacy German rows still resolve"
        );
        assert_eq!(
            german.supplemental_contradictions, 1,
            "the canonical mutation disagrees with its source"
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

fn coverage(fixture: &Fixture, catalog: &Value) -> TestResult<(Coverage, Coverage)> {
    let fields = ["name", "description", "displayType"].map(str::to_owned);
    real_archive_canonical_texts::kind_coverage(
        catalog,
        Kind::Items,
        &fields,
        &fixture.english,
        &fixture.german,
    )
}

fn database() -> DataForgeBuilder {
    let localization = TestInstance::new(LOCALIZATION)
        .value("Name", TestValue::text("@shared"))
        .value("Description", TestValue::text("@resource_description"));
    let definition =
        TestInstance::new(ATTACH_DEFINITION).value("Localization", TestValue::Struct(localization));
    let attachment =
        TestInstance::new(ATTACHMENT).value("AttachDef", TestValue::Struct(definition));
    let purchasable =
        TestInstance::new(PURCHASE).value("displayType", TestValue::text("@own_type"));
    let item = TestRecord::new(
        1,
        OWNER,
        "EntityClassDefinition.ItemPresentation",
        "libs/foundry/records/entities/scitem/fixture/item_presentation.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Struct(attachment),
            TestValue::Struct(purchasable),
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
