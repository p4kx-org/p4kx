// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, InputScope, RecordTree, SourceInput, SourceLocation,
    SourceUseRole, SourceUses, Value as Raw,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::canonical_header_fixture::{self as fixture, MetadataTag};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory as inventory;

#[test]
fn canonical_header_metadata_keeps_exact_plain_encrypted_and_unknown_source_cells() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, json!(false)),
        (DcbOrigin::Encrypted, json!(true)),
        (DcbOrigin::Unknown, Value::Null),
    ] {
        let (_, catalog) = fixture::build(
            &fixture::database(MetadataTag::Present),
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        for (guid, table, _) in fixture::records() {
            let row = fixture::row(&catalog, table, guid)?;
            let index = common::at(row, "/recordIndex/value")?
                .as_u64()
                .ok_or("physical header index")?;
            for (field, source) in [("fileName", "fileName"), ("metadataTag", "metadataTag")] {
                let carrier = common::at(row, &format!("/{field}"))?;
                assert_eq!(
                    common::at(carrier, "/containsEncryptedSource")?,
                    &marker,
                    "{table}/{field} uses only its actual DCB input"
                );
                assert_eq!(
                    common::at(carrier, "/sourceSetComplete")?,
                    &json!(true),
                    "source inventory completeness stays distinct from encryption certainty"
                );
                assert_eq!(
                    fixture::header_uses(&catalog, carrier)?,
                    BTreeSet::from([(index, source.to_owned(), "payload".to_owned())]),
                    "metadata fields keep their own exact original source selector"
                );
            }
        }
        inventory::reference_closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn canonical_metadata_absence_and_no_string_keep_their_actual_source_roles() -> TestResult {
    for (tag, role) in [
        (MetadataTag::V6, "selector"),
        (MetadataTag::Null, "payload"),
        (MetadataTag::Empty, "payload"),
    ] {
        let (_, catalog) = fixture::build(
            &fixture::database(tag),
            DcbOrigin::Plain,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let row = fixture::row(&catalog, "definitions/tags", fixture::TAG)?;
        let index = common::at(row, "/recordIndex/value")?
            .as_u64()
            .ok_or("physical tag header")?;
        let carrier = common::at(row, "/metadataTag")?;
        assert_eq!(
            fixture::header_uses(&catalog, carrier)?,
            BTreeSet::from([(index, "metadataTag".to_owned(), role.to_owned())]),
            "layout absence is a selector and the stored no-string sentinel is a retained physical fact"
        );
        assert_eq!(
            common::at(carrier, "/containsEncryptedSource")?,
            &json!(false),
            "a complete original plain input remains independently known"
        );
        assert_eq!(
            common::at(carrier, "/sourceSetComplete")?,
            &json!(true),
            "the original successful metadata query is complete"
        );
    }
    Ok(())
}

#[test]
fn canonical_header_and_literal_dollar_name_body_field_have_distinct_original_source_coordinates()
-> TestResult {
    let source = source_with_body_header_labels();
    let (_folder, archive) = common::open(&source, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let record = forge.record(0).ok_or("original Record")?;
    let tree = record.tree()?;
    assert_original_names(&tree);
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = fixture::row(&catalog, "definitions/resourceGroups", fixture::GROUP)?;
    let carrier = common::at(row, "/recordName")?;
    assert_separate_locations(&entry, record.index(), &catalog, carrier)?;
    Ok(())
}

#[test]
fn canonical_derived_header_values_use_actual_filename_and_name_branch_inputs() -> TestResult {
    for missing_name in [false, true] {
        let row = TestRecord::new(
            fixture::GROUP,
            "ResourceTypeGroup",
            "ResourceTypeGroup.Header",
            "libs/foundry/records/resourcetypedatabase/Header.XML",
        );
        let row = if missing_name {
            row.without_name()
        } else {
            row
        };
        let source = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new("ResourceTypeGroup"))
            .record(row);
        let (_, catalog) = fixture::build(
            &source,
            DcbOrigin::Plain,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let row = fixture::row(&catalog, "definitions/resourceGroups", fixture::GROUP)?;
        assert_eq!(
            fixture::header_uses(&catalog, common::at(row, "/recordPath")?)?,
            BTreeSet::from([(0, "fileName".to_owned(), "conversion".to_owned())]),
            "extension removal is a conversion of the actual filename, not a fictitious stored recordPath"
        );
        let class = fixture::header_uses(&catalog, common::at(row, "/className")?)?;
        let expected = if missing_name {
            BTreeSet::from([(0, "recordName".to_owned(), "selector".to_owned())])
        } else {
            BTreeSet::from([
                (0, "recordName".to_owned(), "conversion".to_owned()),
                (0, "sourceType".to_owned(), "selector".to_owned()),
            ])
        };
        assert_eq!(
            class, expected,
            "the null-name empty default does not inherit an unused type selector"
        );
    }
    Ok(())
}

fn source_with_body_header_labels() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ResourceTypeGroup")
                .field("$name", TestType::String)
                .field("recordName", TestType::String),
        )
        .record(
            TestRecord::new(
                fixture::GROUP,
                "ResourceTypeGroup",
                "ResourceTypeGroup.HeaderName",
                "libs/foundry/records/resourcetypedatabase/Header.XML",
            )
            .value("$name", TestValue::text("BodyName"))
            .value("recordName", TestValue::text("SameLabelBody")),
        )
}

fn assert_original_names(tree: &RecordTree<'_>) {
    assert_eq!(
        tree.name(),
        "ResourceTypeGroup.HeaderName",
        "header name stays distinct from a literal body property"
    );
    assert_eq!(
        tree.field("$name"),
        Some(&Raw::String("BodyName")),
        "the actual original body field retains its literal name"
    );
    assert_eq!(
        tree.field("recordName"),
        Some(&Raw::String("SameLabelBody")),
        "the same label is also retained as a genuine original body field"
    );
}

fn assert_separate_locations(
    entry: &DataForgeEntry,
    record_index: u32,
    catalog: &Value,
    carrier: &Value,
) -> TestResult {
    let uses = SourceUses::new();
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("the synthetic original Archive binding".into());
    };
    let scope = uses.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let body = scope.at(
        SourceLocation::DataForge {
            record_index,
            field: "$name".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let same_label_body = scope.at(
        SourceLocation::DataForge {
            record_index,
            field: "recordName".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let header = scope.at(
        SourceLocation::Subresource {
            namespace: "dataforgeRecordHeader".to_owned(),
            indices: vec![u64::from(record_index)],
            field: "recordName".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    assert_ne!(
        same_label_body.reference(),
        header.reference(),
        "equal field labels in different original header/body scopes remain distinct"
    );
    assert_ne!(
        body.reference(),
        header.reference(),
        "the common registry does not merge equal body/header labels within one original input"
    );
    let rows = fixture::use_rows(catalog, carrier)?;
    assert_eq!(
        rows.len(),
        1,
        "the raw header name has one direct original payload fact"
    );
    assert_eq!(
        common::at(
            rows.first().ok_or("actual producer header use")?,
            "/location"
        )?,
        &serde_json::to_value(header.description().location())?,
        "the actual producer uses the separate bound header coordinate rather than prefix-dispatching a body string"
    );
    Ok(())
}

#[path = "canonical_header_branches.rs"]
mod branches;
