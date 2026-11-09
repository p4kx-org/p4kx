// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, write_build_manifest,
};
use serde_json::Value;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

const MANUFACTURERS: [(u128, &str, &str, &str); 8] = [
    (
        0xf000_0000_0000_4000_8000_0000_0000_0001,
        "RSI",
        "scitemmanufacturer/scitemmanufacturer.rsi",
        "RSI",
    ),
    (
        0x1000_0000_0000_4000_8000_0000_0000_0002,
        "ACAS",
        "scitemmanufacturer/acas",
        "ACAS",
    ),
    (
        0x8000_0000_0000_4000_8000_0000_0000_0003,
        "Paint_Logo",
        "scitemmanufacturer/paintcolorlogos/paint_logo",
        "",
    ),
    (
        0x2000_0000_0000_4000_8000_0000_0000_0004,
        "Contestant",
        "scitemmanufacturer/contestant",
        "CTST",
    ),
    (
        0x3000_0000_0000_4000_8000_0000_0000_0005,
        "Paint_Test_01",
        "scitemmanufacturer/paint_test_01",
        "",
    ),
    (
        0x4000_0000_0000_4000_8000_0000_0000_0006,
        "Base",
        "scitemmanufacturer/templates/manufacturer_template",
        "",
    ),
    (
        0x5000_0000_0000_4000_8000_0000_0000_0007,
        "DUMMY",
        "scitemmanufacturer/placeholder_corp",
        "",
    ),
    (
        0x6000_0000_0000_4000_8000_0000_0000_0008,
        "Elsewhere",
        "scitemmanufacturers_old/elsewhere",
        "ELSE",
    ),
];

fn database() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        )
        .structure(TestStruct::new("ShopFranchise").field("Code", TestType::String))
        .record(
            TestRecord::new(
                0x7000_0000_0000_4000_8000_0000_0000_0009,
                "ShopFranchise",
                "ShopFranchise.Shop",
                "libs/foundry/records/scitemmanufacturer/shop/shop.xml",
            )
            .value("Code", TestValue::text("SHOP")),
        );
    MANUFACTURERS
        .into_iter()
        .fold(builder, |builder, (guid, class, path, code)| {
            builder.record(
                TestRecord::new(
                    guid,
                    "SCItemManufacturer",
                    format!("SCItemManufacturer.{class}"),
                    format!("libs/foundry/records/{path}.xml"),
                )
                .value("Code", TestValue::text(code))
                .value("Localization", localization(class, code)),
            )
        })
}

fn localization(class: &str, code: &str) -> TestValue {
    let key = |kind: &str| {
        if code.is_empty() {
            TestValue::text("")
        } else {
            TestValue::text(format!("@manufacturer_{kind}{class}"))
        }
    };
    TestValue::Struct(
        TestInstance::new("SCItemLocalization")
            .value("Name", key("Name"))
            .value("Description", key("Desc")),
    )
}

pub(super) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let channel = dir.join("PTU");
    std::fs::create_dir(&channel)?;
    write_build_manifest(
        &channel,
        "sc-alpha-4.4.0",
        "4.4.162.12345",
        "10012345",
        "0123456789abcdef",
    )?;
    let path = channel.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    Ok(path)
}

pub(super) fn catalog_with(
    options: &CatalogOptions<'_>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let built = build_catalog(&archive, options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

pub(super) fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
}

pub(super) fn class_names(catalog: &Value) -> Result<Vec<&str>, Box<dyn Error>> {
    catalog
        .pointer("/definitions/manufacturers/rows")
        .and_then(Value::as_array)
        .ok_or("manufacturer rows")?
        .iter()
        .map(|entry| {
            entry
                .pointer("/className/value")
                .and_then(Value::as_str)
                .ok_or_else(|| "manufacturer className".into())
        })
        .collect()
}
