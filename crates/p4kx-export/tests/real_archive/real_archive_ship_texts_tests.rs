// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

use super::{Fixture, TestResult, replace_value};

#[test]
fn the_ship_baseline_counts_name_and_description_and_checks_role_and_career() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let (english, german) = fixture.coverage(&fixture.catalog, Kind::Ships)?;
    assert_eq!((english.total(), german.total()), (2, 2));
    assert_eq!((english.resolved, german.resolved), (1, 1));
    assert_eq!((english.fallback, german.fallback), (1, 1));
    assert_eq!(
        (
            english.supplemental_contradictions,
            german.supplemental_contradictions
        ),
        (0, 0)
    );

    let mut wrong = fixture.catalog.clone();
    let labels = wrong
        .pointer_mut("/texts/publicNames")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("public names")?;
    for (label, lang) in [("@ship_role_missing", "en"), ("@ship_career_missing", "de")] {
        let target = labels
            .iter_mut()
            .find(|row| row["key"]["value"] == label)
            .ok_or("original ship label")?;
        let display = target
            .pointer_mut(&format!("/translations/value/{lang}/displayText"))
            .ok_or("display form")?;
        replace_value(display, "/state", json!("present"))?;
        replace_value(display, "/value", json!("Invented label"))?;
    }
    let (english, german) = fixture.coverage(&wrong, Kind::Ships)?;
    assert_eq!((english.total(), german.total()), (2, 2));
    assert_eq!((english.contradictions, german.contradictions), (0, 0));
    assert_eq!(
        (
            english.supplemental_contradictions,
            german.supplemental_contradictions
        ),
        (1, 1)
    );
    Ok(())
}

#[test]
fn baselined_ship_rows_require_verifiable_text_occurrences() -> TestResult {
    let fixture = Fixture::from_database(&database())?;
    let mut catalog = fixture.catalog.clone();
    let row = catalog
        .pointer_mut("/ships/0")
        .ok_or("selected Ship fixture")?;
    for field in ["name", "description"] {
        let carrier = row.get_mut(field).ok_or("required Ship text field")?;
        replace_value(carrier, "/state", json!("absent"))?;
        replace_value(carrier, "/value", serde_json::Value::Null)?;
    }
    let (english, german) = fixture.coverage(&catalog, Kind::Ships)?;
    assert_eq!((english.total(), german.total()), (0, 0));
    let mut failures = Vec::new();
    super::check_coverage(Kind::Ships, &english, &german, true, 1, &mut failures);
    assert_eq!(
        failures.len(),
        1,
        "a populated Ship table requires text coverage"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("DataForgeComponentParams")
                .field("vehicleName", TestType::Locale)
                .field("vehicleDescription", TestType::Locale)
                .field("vehicleRole", TestType::Locale)
                .field("vehicleCareer", TestType::Locale),
        )
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .record(
            TestRecord::new(
                1,
                "EntityClassDefinition",
                "EntityClassDefinition.ShipTextFixture",
                "libs/foundry/records/entities/spaceships/ship_text_fixture.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("VehicleComponentParams")
                        .value("vehicleName", TestValue::text("@shared"))
                        .value("vehicleDescription", TestValue::text("@ship_desc_missing"))
                        .value("vehicleRole", TestValue::text("@ship_role_missing"))
                        .value("vehicleCareer", TestValue::text("@ship_career_missing")),
                )]),
            ),
        )
}
