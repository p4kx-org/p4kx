// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, Kind};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::canonical_header_fixture::{self as fixture, MetadataTag};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn canonical_headers_share_raw_filename_and_separate_record_metadata_tag_for_all_migrated_families()
-> TestResult {
    let (_, catalog) = fixture::build(
        &fixture::database(MetadataTag::Present),
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    assert_eq!(
        common::at(&catalog, "/schemaVersion")?,
        &json!(3),
        "the only current catalog writer is v3"
    );
    for (guid, table, stored) in fixture::records() {
        let row = fixture::row(&catalog, table, guid)?;
        assert_eq!(
            common::at(row, "/fileName/value")?,
            &json!(stored),
            "{table} retains the exact stored filename"
        );
        assert_eq!(
            common::at(row, "/metadataTag/value")?,
            &json!(fixture::DOMAIN),
            "the Record metadata tag has a distinct canonical owner"
        );
        assert_eq!(
            common::at(row, "/recordPath/value")?,
            &json!(
                stored
                    .strip_suffix(".XML")
                    .or_else(|| stored.strip_suffix(".xml"))
                    .unwrap_or(stored)
            ),
            "existing normalized path meaning is unchanged"
        );
        for field in [
            "guid",
            "recordName",
            "className",
            "recordPath",
            "recordIndex",
            "sourceType",
            "fileName",
            "metadataTag",
        ] {
            assert!(
                row.get(field)
                    .is_some_and(|value| value.get("containsEncryptedSource").is_some()
                        && value.get("state").is_some()),
                "{table}/{field} is an immediate sourced datum"
            );
        }
    }
    assert_eq!(
        common::at(
            fixture::row(&catalog, "definitions/tags", fixture::TAG)?,
            "/tagName/value"
        )?,
        &json!("PayloadTaxonomy"),
        "metadata tags do not replace the Tag taxonomy's payload name"
    );
    Ok(())
}

#[test]
fn canonical_metadata_tag_keeps_absent_null_empty_and_present_separate() -> TestResult {
    for (tag, state, value) in [
        (MetadataTag::V6, "absent", Value::Null),
        (MetadataTag::Null, "null", Value::Null),
        (MetadataTag::Empty, "present", json!("")),
        (MetadataTag::Present, "present", json!(fixture::DOMAIN)),
    ] {
        let (_, catalog) = fixture::build(
            &fixture::database(tag),
            DcbOrigin::Plain,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        for (guid, table, _) in fixture::records() {
            let row = fixture::row(&catalog, table, guid)?;
            assert_eq!(
                common::at(row, "/metadataTag/state")?,
                &json!(state),
                "{table} retains the original metadata declaration state"
            );
            assert_eq!(
                common::at(row, "/metadataTag/value")?,
                &value,
                "an actual empty string remains present"
            );
        }
    }
    Ok(())
}

#[test]
fn canonical_filename_is_not_reconstructed_by_appending_an_xml_extension() -> TestResult {
    for path in [
        "libs/foundry/records/resourcetypedatabase/Exact.XML",
        "libs/foundry/records/resourcetypedatabase/Exact.xml",
        "libs/foundry/records/resourcetypedatabase/Exact",
    ] {
        let source = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new("ResourceTypeGroup"))
            .record(
                TestRecord::new(
                    fixture::GROUP,
                    "ResourceTypeGroup",
                    "ResourceTypeGroup.Exact",
                    path,
                )
                .without_tag(),
            );
        let (_, catalog) = fixture::build(
            &source,
            DcbOrigin::Plain,
            CatalogPublicationMode::Normal,
            None,
        )?;
        let row = fixture::row(&catalog, "definitions/resourceGroups", fixture::GROUP)?;
        assert_eq!(
            common::at(row, "/fileName/value")?,
            &json!(path),
            "the actual stored suffix or its absence remains exact"
        );
        assert_eq!(
            common::at(row, "/metadataTag/state")?,
            &json!("null"),
            "a tolerated no-string tag does not introduce a publication refusal"
        );
    }
    Ok(())
}

#[test]
fn canonical_record_name_sentinel_does_not_merge_equal_guid_physical_definitions() -> TestResult {
    let source = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ResourceTypeGroup"))
        .record(
            TestRecord::new(
                fixture::GROUP,
                "ResourceTypeGroup",
                "",
                "libs/foundry/records/resourcetypedatabase/First.XML",
            )
            .without_name(),
        )
        .record(TestRecord::new(
            fixture::GROUP,
            "ResourceTypeGroup",
            "",
            "libs/foundry/records/resourcetypedatabase/Second.xml",
        ));
    let (built, catalog) = fixture::build(
        &source,
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    let rows = common::at(&catalog, "/definitions/resourceGroups/rows")?
        .as_array()
        .ok_or("physical group rows")?;
    assert_eq!(
        rows.len(),
        2,
        "equal nominal GUIDs retain both physical definitions"
    );
    for (index, state) in [(0, "null"), (1, "present")] {
        let row = rows.get(index).ok_or("original physical row")?;
        assert_eq!(
            common::at(row, "/recordIndex/value")?,
            &json!(index),
            "source physical order stays unchanged"
        );
        assert_eq!(
            common::at(row, "/recordName/state")?,
            &json!(state),
            "the actual no-string sentinel differs from stored empty text"
        );
        assert_eq!(
            common::at(row, "/className/value")?,
            &json!(""),
            "the existing convenience class-name result stays unchanged"
        );
    }
    assert_eq!(
        built
            .summary()
            .kind(Kind::ResourceGroups)
            .ok_or("group count")?
            .count,
        2,
        "header reuse does not inflate or merge counts"
    );
    Ok(())
}

#[test]
fn canonical_header_schema_requires_the_shared_fields_on_every_migrated_family() -> TestResult {
    let (_, catalog) = fixture::build(
        &fixture::database(MetadataTag::Present),
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        None,
    )?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the actual v3 producer is the positive control"
    );
    for (_, table, _) in fixture::records() {
        for field in [
            "fileName",
            "metadataTag",
            "recordName",
            "recordIndex",
            "sourceType",
        ] {
            let mut invalid = catalog.clone();
            invalid
                .pointer_mut(&format!("/{table}/rows/0"))
                .and_then(Value::as_object_mut)
                .ok_or("canonical header owner")?
                .remove(field)
                .ok_or("required shared header field")?;
            assert!(
                !validator.is_valid(&invalid),
                "{table}/{field} is required by the canonical header contract"
            );
        }
    }
    Ok(())
}

#[test]
fn canonical_metadata_tag_does_not_expand_the_entity_filter_scope() -> TestResult {
    let filter = Filter::new(fixture::DOMAIN);
    let (_, catalog) = fixture::build(
        &fixture::database(MetadataTag::Present),
        DcbOrigin::Plain,
        CatalogPublicationMode::DiagnosticPartial,
        Some(&filter),
    )?;
    for table in [
        "declarations/entityTags",
        "definitions/manufacturers",
        "definitions/resourceGroups",
        "definitions/resourceTypes",
        "definitions/commodities",
    ] {
        assert!(
            common::at(&catalog, &format!("/{table}/rows"))?
                .as_array()
                .ok_or("filtered rows")?
                .is_empty(),
            "metadata tags do not select owning Records or health-only dependencies for {table}"
        );
    }
    assert_eq!(
        common::at(&catalog, "/definitions/tags/rows")?
            .as_array()
            .map(Vec::len),
        Some(1),
        "global Tag taxonomy retains its separate selection scope"
    );
    let tag = fixture::row(&catalog, "definitions/tags", fixture::TAG)?;
    assert_eq!(
        common::at(tag, "/metadataTag/value")?,
        &json!(fixture::DOMAIN),
        "the global definition still exposes its separate metadata tag"
    );
    Ok(())
}
