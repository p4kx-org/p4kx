// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, Kind};
use p4kx_testkit::{DataForgeBuilder, TestStruct};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::DcbOrigin;
use crate::manufacturer_v3_fixture::{self as fixture, TestResult};

#[test]
fn the_writer_emits_only_the_canonical_sourced_manufacturer_definition() -> TestResult {
    let (built, catalog) = fixture::normal(&fixture::database())?;
    assert!(
        catalog.get("manufacturers").is_none(),
        "no root legacy view is emitted"
    );
    assert_eq!(fixture::at(&catalog, "/schemaVersion")?, &json!(3));
    assert_eq!(
        fixture::at(&catalog, "/definitions/manufacturers/availability")?,
        &json!("available")
    );
    let row = fixture::row(&catalog)?;
    assert_header(row)?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::Manufacturers)
            .ok_or("manufacturer summary")?
            .count,
        1
    );
    let schema = p4kx_export::catalog_schema();
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&catalog),
        "the actual writer satisfies the generated schema"
    );
    Ok(())
}

fn assert_header(row: &Value) -> TestResult {
    assert_eq!(
        fixture::at(row, "/guid/value")?,
        &json!(fixture::guid(fixture::MAKER)),
        "the authored GUID is retained"
    );
    assert_eq!(
        fixture::at(row, "/recordIndex/value")?,
        &json!(0),
        "the original physical ordinal is retained"
    );
    assert_eq!(
        fixture::at(row, "/recordName/value")?,
        &json!("SCItemManufacturer.Synthetic"),
        "the unstripped Record name is retained"
    );
    assert_eq!(
        fixture::at(row, "/className/value")?,
        &json!("Synthetic"),
        "the actual class-name conversion is retained"
    );
    assert_eq!(
        fixture::at(row, "/code/value")?,
        &json!(" SYN "),
        "the authored code keeps its whitespace"
    );
    assert_eq!(
        fixture::at(row, "/localization/value/sourceType/value")?,
        &json!("ManufacturerLocalization"),
        "the actual owned struct type is retained"
    );
    assert_eq!(
        fixture::at(row, "/sourceType/value")?,
        &json!("SCItemManufacturer"),
        "the original Record source type is retained"
    );
    Ok(())
}

#[test]
fn source_family_availability_is_independent_of_selected_row_count() -> TestResult {
    let empty: &[Value] = &[];
    let absent = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty"));
    let (_, catalog) = fixture::normal(&absent)?;
    assert_eq!(
        fixture::at(&catalog, "/definitions/manufacturers/availability")?,
        &json!("absent")
    );
    assert_eq!(
        fixture::rows(&catalog)?.as_slice(),
        empty,
        "the measured selection emits no definitions"
    );
    let declared = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SCItemManufacturer"));
    let (_, catalog) = fixture::normal(&declared)?;
    assert_eq!(
        fixture::at(&catalog, "/definitions/manufacturers/availability")?,
        &json!("available")
    );
    assert_eq!(
        fixture::rows(&catalog)?.as_slice(),
        empty,
        "the measured selection emits no definitions"
    );
    let filter = Filter::new("never_selected");
    let (built, catalog) = fixture::build(
        &fixture::database(),
        DcbOrigin::Plain,
        CatalogPublicationMode::Normal,
        Some(&filter),
    )?;
    assert_eq!(
        fixture::at(&catalog, "/definitions/manufacturers/availability")?,
        &json!("available")
    );
    assert_eq!(
        fixture::rows(&catalog)?.as_slice(),
        empty,
        "the measured selection emits no definitions"
    );
    assert!(
        !built
            .summary()
            .kind(Kind::Manufacturers)
            .ok_or("filtered manufacturer summary")?
            .complete
    );
    Ok(())
}

#[test]
fn public_names_and_protected_descriptions_keep_separate_canonical_domains() -> TestResult {
    let (_, catalog) = fixture::normal(&fixture::database())?;
    let public = fixture::text(&catalog, "publicNames", "name")?;
    let protected = fixture::text(&catalog, "gameTexts", "description")?;
    assert_eq!(fixture::at(public, "/key/value")?, &json!("@shared"));
    assert_eq!(fixture::at(protected, "/key/value")?, &json!("@shared"));
    assert_ne!(
        fixture::at(public, "/id/value")?,
        fixture::at(protected, "/id/value")?
    );
    assert_eq!(
        fixture::at(public, "/translations/value/en/value")?,
        &json!("Shared text")
    );
    assert_eq!(
        fixture::at(protected, "/translations/value/de/value")?,
        &json!("Gemeinsamer Text")
    );
    crate::commodity_v3_source_inventory::reference_closure(&catalog)?;
    Ok(())
}

#[test]
fn guid_order_never_replaces_the_original_physical_record_index() -> TestResult {
    let database = fixture::database().record(fixture::record(
        0x10,
        "Earlier",
        "libs/foundry/records/scitemmanufacturer/included/earlier.xml",
    ));
    let (_, catalog) = fixture::normal(&database)?;
    let rows = fixture::rows(&catalog)?;
    assert_eq!(rows.len(), 2);
    let first = rows.first().ok_or("first sorted Manufacturer")?;
    assert_eq!(
        fixture::at(first, "/guid/value")?,
        &json!(fixture::guid(0x10))
    );
    assert_eq!(fixture::at(first, "/recordIndex/value")?, &json!(1));
    let second = rows.get(1).ok_or("second sorted Manufacturer")?;
    assert_eq!(
        fixture::at(second, "/guid/value")?,
        &json!(fixture::guid(fixture::MAKER))
    );
    assert_eq!(fixture::at(second, "/recordIndex/value")?, &json!(0));
    Ok(())
}

#[test]
fn repeat_preparation_keeps_the_complete_catalog_bytes_deterministic() -> TestResult {
    let (_directory, archive) =
        crate::commodity_v3_fixture::open(&fixture::database(), DcbOrigin::Plain)?;
    let options = crate::commodity_v3_fixture::options();
    let first = p4kx_export::build_catalog(&archive, &options)?;
    let second = p4kx_export::build_catalog(&archive, &options)?;
    assert_eq!(
        first.json(),
        second.json(),
        "parallel text/source preparation retains stable bytes and SourceUse IDs"
    );
    Ok(())
}
