// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{BuiltCatalog, CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as archive_fixture, DcbOrigin, TestResult};

pub(crate) const TAG: u128 = 0x301;
pub(crate) const ROOTS: u128 = 0x302;
pub(crate) const MAKER: u128 = 0x303;
pub(crate) const GROUP: u128 = 0x304;
pub(crate) const RESOURCE: u128 = 0x305;
pub(crate) const ENTITY: u128 = 0x306;
pub(crate) const DOMAIN: &str = "RecordDomain";

#[derive(Clone, Copy)]
pub(crate) enum MetadataTag {
    V6,
    Null,
    Empty,
    Present,
}

impl MetadataTag {
    pub(crate) const fn version(self) -> u32 {
        if matches!(self, Self::V6) { 6 } else { 8 }
    }
    pub(crate) fn apply(self, record: TestRecord) -> TestRecord {
        match self {
            Self::Null => record.without_tag(),
            Self::Empty => record.tag(""),
            Self::V6 | Self::Present => record.tag(DOMAIN),
        }
    }
}

pub(crate) const fn records() -> [(u128, &'static str, &'static str); 7] {
    [
        (TAG, "definitions/tags", "synthetic/HeaderTag.XML"),
        (ROOTS, "declarations/tagRoots", "synthetic/HeaderRoots.xml"),
        (
            ENTITY,
            "declarations/entityTags",
            "libs/foundry/records/entities/commodities/included/Commodity.XML",
        ),
        (
            MAKER,
            "definitions/manufacturers",
            "libs/foundry/records/scitemmanufacturer/included/Maker.XML",
        ),
        (
            GROUP,
            "definitions/resourceGroups",
            "libs/foundry/records/resourcetypedatabase/included/Group.XML",
        ),
        (
            RESOURCE,
            "definitions/resourceTypes",
            "libs/foundry/records/resourcetypedatabase/included/Resource.XML",
        ),
        (
            ENTITY,
            "definitions/commodities",
            "libs/foundry/records/entities/commodities/included/Commodity.XML",
        ),
    ]
}

fn declarations(tag: MetadataTag) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(tag.version())
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(TestStruct::new("SCItemManufacturer"))
        .structure(TestStruct::new("ResourceTypeGroup"))
        .structure(TestStruct::new("ResourceType"))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
}

pub(crate) fn database(tag: MetadataTag) -> DataForgeBuilder {
    let mut source = declarations(tag);
    for (guid, kind, name, path) in [
        (TAG, "Tag", "Tag.Header", "synthetic/HeaderTag.XML"),
        (
            ROOTS,
            "TagDatabase",
            "TagDatabase.Header",
            "synthetic/HeaderRoots.xml",
        ),
        (
            MAKER,
            "SCItemManufacturer",
            "SCItemManufacturer.Header",
            "libs/foundry/records/scitemmanufacturer/included/Maker.XML",
        ),
        (
            GROUP,
            "ResourceTypeGroup",
            "ResourceTypeGroup.Header",
            "libs/foundry/records/resourcetypedatabase/included/Group.XML",
        ),
        (
            RESOURCE,
            "ResourceType",
            "ResourceType.Header",
            "libs/foundry/records/resourcetypedatabase/included/Resource.XML",
        ),
        (
            ENTITY,
            "EntityClassDefinition",
            "EntityClassDefinition.Header",
            "libs/foundry/records/entities/commodities/included/Commodity.XML",
        ),
    ] {
        let mut record = TestRecord::new(guid, kind, name, path);
        if guid == TAG {
            record = record
                .value("tagName", TestValue::text("PayloadTaxonomy"))
                .value("legacyGUID", TestValue::UInt32(1));
        }
        if guid == ROOTS || guid == ENTITY {
            record = record.value("tags", TestValue::Array(vec![TestValue::Reference(TAG)]));
        }
        source = source.record(tag.apply(record));
    }
    source
}

pub(crate) fn build(
    source: &DataForgeBuilder,
    origin: DcbOrigin,
    mode: CatalogPublicationMode,
    filter: Option<&Filter>,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_folder, archive) = archive_fixture::open(source, origin)?;
    let mut options = archive_fixture::options();
    options.publication_mode = mode;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

pub(crate) fn row<'a>(catalog: &'a Value, table: &str, guid: u128) -> TestResult<&'a Value> {
    let guid = archive_fixture::guid(guid);
    let rows = catalog
        .pointer(&format!("/{table}/rows"))
        .and_then(Value::as_array)
        .ok_or("canonical rows")?;
    let mut matching = rows
        .iter()
        .filter(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(guid.as_str()));
    let row = matching.next().ok_or("selected physical definition")?;
    if matching.next().is_some() {
        return Err("the fixture GUID must be unique".into());
    }
    Ok(row)
}

pub(crate) fn use_rows<'a>(catalog: &'a Value, carrier: &Value) -> TestResult<Vec<&'a Value>> {
    let inventory = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    let references = carrier
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("immediate source references")?;
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for reference in references {
        let id = reference.as_str().ok_or("source-use reference")?;
        assert!(
            seen.insert(id),
            "an immediate source set has no duplicate references"
        );
        let mut matching = inventory
            .iter()
            .filter(|row| row.get("id").and_then(Value::as_str) == Some(id));
        rows.push(matching.next().ok_or("closed source use")?);
        assert!(
            matching.next().is_none(),
            "a source use has one inventory owner"
        );
    }
    Ok(rows)
}

pub(crate) fn header_uses(
    catalog: &Value,
    carrier: &Value,
) -> TestResult<BTreeSet<(u64, String, String)>> {
    let sources = catalog
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("original physical sources")?;
    let mut result = BTreeSet::new();
    for row in use_rows(catalog, carrier)? {
        assert_eq!(
            row.pointer("/scope").and_then(Value::as_str),
            Some("payload"),
            "Record headers are original DCB payload facts"
        );
        assert_eq!(
            row.pointer("/location/type").and_then(Value::as_str),
            Some("subresource"),
            "header coordinates remain separate from body property paths"
        );
        assert_eq!(
            row.pointer("/location/namespace").and_then(Value::as_str),
            Some("dataforgeRecordHeader"),
            "the owned header namespace is explicit"
        );
        let indices = row
            .pointer("/location/indices")
            .and_then(Value::as_array)
            .ok_or("header indices")?;
        assert_eq!(
            indices.len(),
            1,
            "one original physical Record position addresses the header"
        );
        let index = indices
            .first()
            .and_then(Value::as_str)
            .ok_or("decimal original Record index")?
            .parse::<u32>()?;
        assert_original_archive_input(sources, row)?;
        result.insert((
            u64::from(index),
            row.pointer("/location/field")
                .and_then(Value::as_str)
                .ok_or("header field")?
                .to_owned(),
            row.get("role")
                .and_then(Value::as_str)
                .ok_or("source role")?
                .to_owned(),
        ));
    }
    Ok(result)
}

/// Checks the original Archive DCB binding of one observed use.
fn assert_original_archive_input(sources: &[Value], row: &Value) -> TestResult {
    let source_id = row.get("source").ok_or("physical source reference")?;
    let source = sources
        .iter()
        .find(|source| source.get("id") == Some(source_id))
        .ok_or("closed physical source")?;
    assert_eq!(
        source.get("kind").and_then(Value::as_str),
        Some("archiveEntry"),
        "the header retains its original physical Archive input"
    );
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some("Data\\Game2.dcb"),
        "no locale or reconstructed host input supplies a Record header"
    );
    Ok(())
}
