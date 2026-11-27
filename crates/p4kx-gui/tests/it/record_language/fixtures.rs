// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic DataForge whose Records hold localisation keys, in Archives with English and
//! German `global.ini` files.

use std::error::Error;
use std::path::PathBuf;

use p4kx_gui::TextLanguage;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};
use tempfile::TempDir;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

const CART: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const TRUCK: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;

pub(crate) const TRUCK_PATH: &str = "libs/vehicles/truck";

const ENGLISH: &str = "Data\\Localization\\english\\global.ini";
const GERMAN: &str = "Data\\Localization\\german_(germany)\\global.ini";
const FRENCH: &str = "Data\\Localization\\french_(france)\\global.ini";

/// A Cart and a Truck, each with a name, a description, a placeholder, an empty key and a list
/// of keys.
pub(crate) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Vehicle")
                .field("name", TestType::Locale)
                .field("description", TestType::Locale)
                .field("unset", TestType::Locale)
                .field("empty", TestType::Locale)
                .array("labels", TestType::Locale),
        )
        .record(vehicle(CART, "Cart", "@vehicle_Cart", "@vehicle_CartDesc"))
        .record(vehicle(
            TRUCK,
            "Truck",
            "@vehicle_Truck",
            "@vehicle_TruckDesc",
        ))
}

fn vehicle(guid: u128, name: &str, key: &str, description: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "Vehicle",
        format!("Vehicle.{name}"),
        format!("libs/vehicles/{}.xml", name.to_lowercase()),
    )
    .value("name", TestValue::text(key))
    .value("description", TestValue::text(description))
    .value("unset", TestValue::text("@LOC_PLACEHOLDER"))
    .value("empty", TestValue::text(""))
    .value(
        "labels",
        TestValue::Array(vec![
            TestValue::text(key),
            TestValue::text("@vehicle_Unknown"),
        ]),
    )
}

/// The English file: every text, the description with a line break.
pub(crate) fn english() -> Vec<u8> {
    GlobalIniBuilder::new()
        .text("vehicle_Cart", "Cart")
        .text("vehicle_CartDesc", "Rolls.\\nSlowly.")
        .text("vehicle_Truck", "Truck")
        .text("vehicle_TruckDesc", "Hauls.")
        .build()
}

/// The German file: no description of the Cart, and a Truck name with control and bidirectional
/// characters.
pub(crate) fn german() -> Vec<u8> {
    GlobalIniBuilder::new()
        .text("vehicle_Cart", "Karren")
        .text("vehicle_Truck", "Laster\u{202E}\u{1b}x")
        .text("vehicle_TruckDesc", "Zieht.")
        .build()
}

/// Writes an Archive with `database` and the `global.ini` files `files`.
pub(crate) fn archive_with(
    files: &[(&str, Vec<u8>)],
    extra: Vec<TestEntry>,
) -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let mut builder =
        ArchiveBuilder::new().entry(TestEntry::zstd("Data\\Game2.dcb", database().build()));
    for (entry, bytes) in files {
        builder = builder.entry(TestEntry::zstd(*entry, bytes.clone()));
    }
    for entry in extra {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok((dir, path))
}

/// An Archive with the English and the German `global.ini`.
pub(crate) fn archive() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    archive_with(&[(ENGLISH, english()), (GERMAN, german())], Vec::new())
}

/// An Archive with the English `global.ini`, a French one that cannot be read, and no German.
pub(crate) fn archive_with_broken_french() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let text = b"vehicle_Cart=Charrette\n";
    let broken = TestEntry::zstd(FRENCH, text.as_slice())
        .with_uncompressed_size(u64::try_from(text.len())? - 1);
    archive_with(&[(ENGLISH, english())], vec![broken])
}

/// An Archive without any `global.ini`.
pub(crate) fn archive_without_texts() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    archive_with(&[], Vec::new())
}

/// The languages of [`archive`].
pub(crate) const ENGLISH_AND_GERMAN: [TextLanguage; 2] =
    [TextLanguage::English, TextLanguage::German];
