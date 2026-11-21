// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoDefinitionSnapshot, AmmoSelection, with_ammo_definitions, with_ammo_definitions_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForgeEntry, Language};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const AMMO_GUID: u128 = 0x91a0;
const ALIAS: u32 = 1;
const UNREADABLE: u32 = 2;
const LANGUAGES: &[Language] = &[Language::English, Language::German];
const AMMO_ROWS: &str = "/definitions/ammoDefinitions/rows";

#[test]
fn physical_ammo_definition_matches_the_canonical_row_and_localization() -> TestResult {
    let database = database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..common::options()
        };
        let catalog = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
        with_ammo_definitions(
            &archive,
            CatalogRecordSelection::RecordIndex(ALIAS),
            CatalogLanguageSelection::Selected(LANGUAGES),
            None,
            |snapshot| verify_row(&snapshot, &catalog, encrypted),
        )??;
    }
    Ok(())
}

#[test]
fn ammo_lookup_distinguishes_an_unrelated_unreadable_physical_record() -> TestResult {
    let database =
        database().record(ammo(AMMO_GUID + 1, "unreadable_ammo").with_instance_index(u16::MAX));
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let languages = CatalogLanguageSelection::Selected(&[]);
    with_ammo_definitions_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(ALIAS),
        languages,
        None,
        |snapshot| -> TestResult {
            assert_eq!(snapshot.rows().len(), 1, "the requested alias is readable");
            assert!(
                snapshot.coverage().unreadable_bodies.is_empty(),
                "the query did not reach the other Ammo Record"
            );
            Ok(())
        },
    )??;
    with_ammo_definitions_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(UNREADABLE),
        languages,
        None,
        |snapshot| -> TestResult {
            assert!(
                snapshot.rows().is_empty(),
                "the unreadable body supplied no row"
            );
            assert_eq!(
                snapshot.coverage().unreadable_bodies,
                [UNREADABLE],
                "the same original body is unreadable when requested"
            );
            Ok(())
        },
    )?
}

fn verify_row(
    snapshot: &AmmoDefinitionSnapshot<'_>,
    catalog: &Value,
    encrypted: bool,
) -> TestResult {
    let [row] = snapshot.rows() else {
        return Err("exactly one physical Ammo row".into());
    };
    assert_eq!(
        row.selection(),
        AmmoSelection::Selected,
        "independently admitted Ammo owner"
    );
    assert_eq!(
        row.source().record_index().value(),
        Some(&ALIAS),
        "an equal GUID never substitutes for the requested original Record"
    );
    assert_eq!(
        row.speed()
            .value()
            .map(p4kx_export::catalog_queries::ResourceFloat32::raw_bits),
        Some(42.5_f32.to_bits()),
        "the selected physical alias retains its exact speed"
    );
    assert_eq!(
        row.speed().origin().contains_encrypted_source(),
        Some(encrypted),
        "the source marker belongs to this field's original Entry"
    );
    let expected = proof::rows(catalog, AMMO_ROWS)?
        .iter()
        .find(|row| row.pointer("/recordIndex/value") == Some(&json!(ALIAS)))
        .ok_or("the same physical Ammo Record in the full Catalog")?;
    assert_eq!(
        serde_json::to_value(row)?,
        *expected,
        "every canonical field and use reference"
    );
    verify_localization(snapshot, catalog)
}

fn verify_localization(snapshot: &AmmoDefinitionSnapshot<'_>, catalog: &Value) -> TestResult {
    let [name] = snapshot.public_names() else {
        return Err("one public name for the selected Ammo Record".into());
    };
    assert_eq!(
        snapshot.available_languages(),
        LANGUAGES,
        "only requested present locales are read"
    );
    let actual = serde_json::to_value(name)?;
    let expected = proof::rows(catalog, "/texts/publicNames")?
        .iter()
        .find(|row| row.pointer("/id/value") == actual.pointer("/id/value"))
        .ok_or("the same Ammo public name in the full Catalog")?;
    assert_eq!(
        &actual, expected,
        "the locale values and original uses match"
    );
    proof::closure(&json!({
        "ammunition": snapshot.rows(),
        "publicNames": snapshot.public_names(),
        "provenance": snapshot.source_inventory(),
    }))
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AmmoParams")
                .field("displayName", TestType::Locale)
                .field("speed", TestType::Single),
        )
        .record(
            ammo(AMMO_GUID, "first_ammo")
                .value("displayName", TestValue::text("@shared"))
                .value("speed", TestValue::Single(1.0)),
        )
        .record(
            ammo(AMMO_GUID, "second_ammo")
                .value("displayName", TestValue::text("@own_type"))
                .value("speed", TestValue::Single(42.5)),
        )
}

fn ammo(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("libs/foundry/records/ammoparams/{name}.xml"),
    )
}
