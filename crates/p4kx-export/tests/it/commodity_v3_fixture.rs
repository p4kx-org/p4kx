// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{BuiltCatalog, CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType};
use serde_json::Value;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) const RESOURCE: u128 = 0x01;
pub(crate) const RESOURCE_FALLBACK: u128 = 0x02;
pub(crate) const RESOURCE_UNUSED: u128 = 0x03;
pub(crate) const DANGLING_RESOURCE: u128 = 0x99;
pub(crate) const GROUP_FIRST: u128 = 0xf0;
pub(crate) const GROUP_SECOND: u128 = 0x10;
pub(crate) const QUANTIZATION: u128 = 0x20;
pub(crate) const ALPHA: u128 = 0x101;
pub(crate) const BETA: u128 = 0x102;
pub(crate) const UNLINKED: u128 = 0x103;
pub(crate) const ALPHA_INDEX: u64 = 6;
pub(crate) const BETA_INDEX: u64 = 7;

#[derive(Clone, Copy)]
pub(crate) enum DcbOrigin {
    Plain,
    Encrypted,
    Unknown,
}

pub(crate) fn database() -> DataForgeBuilder {
    crate::commodity_v3_fixture_records::database()
}

pub(crate) fn with_keys(
    own_name: &str,
    own_description: &str,
    resource_name: &str,
) -> DataForgeBuilder {
    crate::commodity_v3_fixture_records::with_keys(own_name, own_description, resource_name)
}

pub(crate) fn dangling_first() -> DataForgeBuilder {
    crate::commodity_v3_fixture_records::dangling_first()
}

pub(crate) fn parent_arrays(declared: bool) -> DataForgeBuilder {
    let entity = TestStruct::new("EntityClassDefinition");
    let entity = if declared {
        entity
            .field(
                "Components",
                TestType::StrongPointer("FixtureEmpty".to_owned()),
            )
            .field(
                "StaticEntityClassData",
                TestType::StrongPointer("FixtureEmpty".to_owned()),
            )
    } else {
        entity
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(entity)
        .structure(TestStruct::new("FixtureEmpty"))
        .record(TestRecord::new(
            ALPHA,
            "EntityClassDefinition",
            "EntityClassDefinition.ParentState",
            "libs/foundry/records/entities/commodities/parent_state.xml",
        ))
}

pub(crate) const fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH)
}

pub(crate) fn build(database: &DataForgeBuilder) -> TestResult<(BuiltCatalog, Value)> {
    let (_folder, archive) = open(database, DcbOrigin::Plain)?;
    let built = build_catalog(&archive, &options())?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

pub(crate) fn unknown(database: &DataForgeBuilder) -> TestResult<Value> {
    let (_folder, archive) = open(database, DcbOrigin::Unknown)?;
    let mut options = options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

pub(crate) fn open(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
) -> TestResult<(tempfile::TempDir, Archive)> {
    open_bytes(database.build(), origin)
}

pub(crate) fn open_bytes(
    bytes: Vec<u8>,
    origin: DcbOrigin,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    let dcb = TestEntry::stored("Data\\Game2.dcb", bytes);
    let dcb = match origin {
        DcbOrigin::Plain => dcb,
        DcbOrigin::Encrypted => dcb.encrypted(),
        DcbOrigin::Unknown => dcb.encrypted().with_encryption_field(7),
    };
    ArchiveBuilder::new()
        .entry(dcb)
        .entry(
            TestEntry::stored(Language::English.global_ini_path(), english().as_bytes())
                .encrypted(),
        )
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            german().as_bytes(),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

const fn english() -> &'static str {
    "resource_name=Resource English\nresource_description=Resource description\nown_description=Own description\nown_type=Own type\nshared=Shared text\nlater_name=Later name\nlater_description=Later description\nlater_type=Later type\nfallback_resource_name=Fallback resource"
}

const fn german() -> &'static str {
    "resource_name=Ressource Deutsch\nresource_description=Ressourcenbeschreibung\nown_description=Eigene Beschreibung\nown_type=Eigener Typ\nshared=Gemeinsamer Text\nfallback_resource_name=Fallback-Ressource"
}

pub(crate) fn guid(value: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        value >> 96,
        (value >> 80) & 0xffff,
        (value >> 64) & 0xffff,
        (value >> 48) & 0xffff,
        value & 0xffff_ffff_ffff
    )
}

pub(crate) fn definition(catalog: &Value, guid: u128) -> TestResult<&Value> {
    row(catalog, "/definitions/commodities/rows", "guid", guid)
}

pub(crate) fn presentation(catalog: &Value, guid: u128) -> TestResult<&Value> {
    row(
        catalog,
        "/projections/commodityPresentation/rows",
        "commodityGuid",
        guid,
    )
}

fn row<'catalog>(
    catalog: &'catalog Value,
    path: &str,
    identity: &str,
    expected: u128,
) -> TestResult<&'catalog Value> {
    let expected = guid(expected);
    catalog
        .pointer(path)
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.get(identity)
                    .and_then(|value| value.get("value"))
                    .and_then(Value::as_str)
                    == Some(expected.as_str())
            })
        })
        .ok_or_else(|| "canonical commodity row is missing".into())
}

pub(crate) fn text_row<'catalog>(
    catalog: &'catalog Value,
    domain: &str,
    id: &Value,
) -> TestResult<&'catalog Value> {
    catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.pointer("/id/value") == Some(id)))
        .ok_or_else(|| "canonical chosen text row is missing".into())
}

pub(crate) fn payload<'catalog>(
    value: &'catalog Value,
    field: &str,
) -> TestResult<&'catalog Value> {
    let carrier = value.get(field).ok_or("sourced field missing")?;
    if carrier.get("state").and_then(Value::as_str) != Some("present") {
        return Err("expected a present source value".into());
    }
    carrier
        .get("value")
        .ok_or_else(|| "source value key missing".into())
}

/// Borrows a required JSON member while keeping explicit null distinct from absence.
pub(crate) fn at<'value>(value: &'value Value, pointer: &str) -> TestResult<&'value Value> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("required fixture path {pointer} is missing").into())
}
