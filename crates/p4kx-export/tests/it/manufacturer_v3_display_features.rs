// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, catalog_schema};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::DcbOrigin;
use crate::health_origin_assertions as proof;
use crate::manufacturer_v3_fixture::{self as fixture, MAKER, TestResult};

const LOCALIZATION: &str = "FixtureManufacturerLocalization";
const DISPLAY: &str = "FixtureManufacturerDisplay";
const AUDIO: &str = "FixtureManufacturerGlobalAudio";
const TARGET: &str = "FixtureOpaqueManufacturerTarget";
const TARGET_GUID: u128 = 0x81;
const LOCALE: &str = "@shared";
const REFERENCES: &[(&str, &str)] = &[
    ("AudioManufacturerTag", "audioManufacturerTag"),
    ("BuildingBlocksStyle", "buildingBlocksStyle"),
    ("DashboardCanvasConfig", "dashboardCanvasConfig"),
    ("LightAmplification", "lightAmplification"),
];

#[derive(Clone, Copy)]
enum Layout {
    Complete,
    Null,
    UnknownLeaf,
}

#[test]
fn all_manufacturer_display_fields_keep_their_actual_values_and_text_domains() -> TestResult {
    let (_, catalog) = fixture::build(
        &database(Layout::Complete),
        DcbOrigin::Encrypted,
        CatalogPublicationMode::Normal,
        None,
    )?;
    let schema = catalog_schema();
    jsonschema::validator_for(&schema)?
        .validate(&catalog)
        .map_err(|error| format!("full manufacturer catalog schema: {error}"))?;
    let row = fixture::row(&catalog)?;
    check_root(&catalog, row)?;
    let localization = proof::at(row, "/localization/value")?;
    let short_name = proof::cell(localization, "shortName", "present")?;
    let public_id = proof::at(short_name, "/value")?;
    let features = proof::at(localization, "/displayFeatures/value")?;
    check_features(&catalog, features, public_id)?;
    check_protection(&schema)?;
    proof::closure(&catalog)
}

fn check_root(catalog: &Value, row: &Value) -> TestResult {
    for (_, field) in REFERENCES {
        let reference = proof::cell(row, field, "present")?;
        assert_eq!(
            proof::at(reference, "/containsEncryptedSource")?,
            &json!(true),
            "the reference's own field uses the actual encrypted DCB"
        );
        let target = proof::at(reference, "/value")?;
        proof::present(
            target,
            "guid",
            &json!(fixture::guid(TARGET_GUID)),
            Some(true),
        )?;
        proof::present(target, "targetType", &json!(TARGET), Some(true))?;
        assert_eq!(
            proof::at(target, "/status")?,
            "present",
            "the actual admitted target is described"
        );
    }
    for (field, value) in [
        ("logo", "  fixture/logo  "),
        ("logoFullColor", "fixture/full"),
        ("logoSimplifiedWhite", ""),
    ] {
        let cell = proof::present(row, field, &json!(value), Some(true))?;
        assert!(
            !proof::uses(catalog, cell)?.is_empty(),
            "each original logo string cites its actual field"
        );
    }
    Ok(())
}

fn check_features(catalog: &Value, features: &Value, public_id: &Value) -> TestResult {
    proof::present(features, "sourceType", &json!(DISPLAY), Some(true))?;
    proof::present(
        features,
        "frontendBackground",
        &json!(" fixture/bg "),
        Some(true),
    )?;
    proof::present(
        features,
        "logoSimplifiedWhite",
        &json!("fixture/white"),
        Some(true),
    )?;
    proof::present(features, "uiPriority", &json!(-19), Some(true))?;
    let protected_id = proof::at(features, "/callout1/value")?;
    for field in ["callout1", "callout2", "callout3", "history"] {
        proof::present(features, field, protected_id, Some(true))?;
    }
    assert_ne!(
        protected_id, public_id,
        "one key has distinct public and protected domains"
    );
    for (domain, id) in [("publicNames", public_id), ("gameTexts", protected_id)] {
        let text = proof::rows(catalog, &format!("/texts/{domain}"))?
            .iter()
            .find(|row| row.pointer("/id/value") == Some(id))
            .ok_or("canonical text row for the authored reference")?;
        proof::present(text, "key", &json!(LOCALE), Some(true))?;
        assert_eq!(
            proof::at(text, "/translations/value/en/displayText/value")?,
            "Shared text",
            "the canonical reference resolves through the common locale service"
        );
    }
    for (field, trigger) in [
        ("locationAudioPlayTrigger", "fixture_start"),
        ("locationAudioStopTrigger", "fixture_stop"),
    ] {
        let trigger_row = proof::at(proof::cell(features, field, "present")?, "/value")?;
        proof::present(trigger_row, "sourceType", &json!(AUDIO), Some(true))?;
        proof::present(trigger_row, "audioTrigger", &json!(trigger), Some(true))?;
    }
    Ok(())
}

fn check_protection(schema: &Value) -> TestResult {
    for field in ["callout1", "callout2", "callout3", "history"] {
        let path = format!("/$defs/ManufacturerDisplayFeatures/properties/{field}/x-gameText");
        assert_eq!(
            proof::at(schema, &path)?,
            &json!(true),
            "each protected carrier is marked"
        );
    }
    assert!(
        schema
            .pointer("/$defs/ManufacturerLocalization/properties/shortName/x-gameText")
            .is_none(),
        "the short name remains public"
    );
    Ok(())
}

#[test]
fn nullable_manufacturer_fields_keep_explicit_nulls_and_no_children() -> TestResult {
    let (_, catalog) = fixture::normal(&database(Layout::Null))?;
    let row = fixture::row(&catalog)?;
    for (_, field) in REFERENCES {
        proof::cell(row, field, "null")?;
    }
    let localization = proof::at(row, "/localization/value")?;
    proof::cell(localization, "displayFeatures", "null")?;
    proof::closure(&catalog)
}

#[test]
fn an_unknown_manufacturer_display_declaration_still_refuses_normal_publication() -> TestResult {
    let Err(error) = fixture::normal(&database(Layout::UnknownLeaf)) else {
        return Err("an additional unknown display declaration must refuse publication".into());
    };
    assert!(
        matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::IncompleteExport(_))
        ),
        "unknown declarations remain a typed publication error"
    );
    Ok(())
}

fn database(layout: Layout) -> DataForgeBuilder {
    let mut declaration = TestStruct::new("SCItemManufacturer")
        .field("Code", TestType::String)
        .field("Localization", class(LOCALIZATION));
    let mut maker = fixture::record(
        MAKER,
        "SyntheticDisplay",
        "libs/foundry/records/scitemmanufacturer/manufacturer_display.xml",
    )
    .value("Code", TestValue::text("Fixture Code"))
    .value("Localization", node(localization(layout)));
    for (name, _) in REFERENCES {
        declaration = declaration.field(*name, TestType::ReferenceTo(TARGET.to_owned()));
        if !matches!(layout, Layout::Null) {
            maker = maker.value(*name, TestValue::Reference(TARGET_GUID));
        }
    }
    for (name, value) in [
        ("Logo", "  fixture/logo  "),
        ("LogoFullColor", "fixture/full"),
        ("LogoSimplifiedWhite", ""),
    ] {
        declaration = declaration.field(name, TestType::String);
        maker = maker.value(name, TestValue::text(value));
    }
    let display_slot = if matches!(layout, Layout::Null) {
        TestType::StrongPointer(DISPLAY.to_owned())
    } else {
        class(DISPLAY)
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(declaration)
        .structure(
            TestStruct::new(LOCALIZATION)
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale)
                .field("ShortName", TestType::Locale)
                .field("displayFeatures", display_slot),
        )
        .structure(display_definition(layout))
        .structure(TestStruct::new(AUDIO).field("audioTrigger", TestType::String))
        .structure(TestStruct::new(TARGET))
        .record(maker)
        .record(TestRecord::new(
            TARGET_GUID,
            TARGET,
            "FixtureTarget",
            "fixture/unknown_target.xml",
        ))
}

fn localization(layout: Layout) -> TestInstance {
    let display = if matches!(layout, Layout::Null) {
        TestValue::Null
    } else {
        node(display(layout))
    };
    TestInstance::new(LOCALIZATION)
        .value("Name", TestValue::text(LOCALE))
        .value("Description", TestValue::text(LOCALE))
        .value("ShortName", TestValue::text(LOCALE))
        .value("displayFeatures", display)
}

fn display_definition(layout: Layout) -> TestStruct {
    let source = TestStruct::new(DISPLAY)
        .field("Callout1", TestType::Locale)
        .field("Callout2", TestType::Locale)
        .field("Callout3", TestType::Locale)
        .field("History", TestType::Locale)
        .field("FrontendBackground", TestType::String)
        .field("LogoSimplifiedWhite", TestType::String)
        .field("UIPriority", TestType::Int32)
        .field("locationAudioPlayTrigger", class(AUDIO))
        .field("locationAudioStopTrigger", class(AUDIO));
    if matches!(layout, Layout::UnknownLeaf) {
        source.field("FutureMember", TestType::UInt32)
    } else {
        source
    }
}

fn display(layout: Layout) -> TestInstance {
    let source = TestInstance::new(DISPLAY)
        .value("Callout1", TestValue::text(LOCALE))
        .value("Callout2", TestValue::text(LOCALE))
        .value("Callout3", TestValue::text(LOCALE))
        .value("History", TestValue::text(LOCALE))
        .value("FrontendBackground", TestValue::text(" fixture/bg "))
        .value("LogoSimplifiedWhite", TestValue::text("fixture/white"))
        .value("UIPriority", TestValue::Int32(-19))
        .value("locationAudioPlayTrigger", audio("fixture_start"))
        .value("locationAudioStopTrigger", audio("fixture_stop"));
    if matches!(layout, Layout::UnknownLeaf) {
        source.value("FutureMember", TestValue::UInt32(0))
    } else {
        source
    }
}

fn audio(value: &str) -> TestValue {
    node(TestInstance::new(AUDIO).value("audioTrigger", TestValue::text(value)))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
