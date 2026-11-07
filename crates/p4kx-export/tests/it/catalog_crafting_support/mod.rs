// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Building and reading the catalog of a synthetic Archive for the crafting catalog tests.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, CatalogProgress, Filter, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry};
use serde_json::Value;

/// Returns the GUID `value` as canonical lower-case UUID text.
pub(crate) fn text(value: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        value >> 96,
        (value >> 80) & 0xffff,
        (value >> 64) & 0xffff,
        (value >> 48) & 0xffff,
        value & 0xffff_ffff_ffff
    )
}

fn write_archive(dir: &Path, forge: &DataForgeBuilder) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new()
                .text("StatName_GPP_Armor", "Armour")
                .build(),
        ))
        .write_to(&path)?;
    Ok(path)
}

/// Builds the catalog of an Archive holding `forge`, and returns it with its JSON.
pub(crate) fn build_from(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let built = build_with(forge, filter, None)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

/// Builds an intentionally incomplete crafting fixture with explicit diagnostic publication.
pub(crate) fn diagnostic_from(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let built = build_with_mode(
        forge,
        filter,
        None,
        p4kx_export::CatalogPublicationMode::DiagnosticPartial,
    )?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

/// Builds the catalog of an Archive holding `forge`, reporting its progress to `progress`.
pub(crate) fn build_with(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
) -> Result<BuiltCatalog, Box<dyn Error>> {
    build_with_mode(
        forge,
        filter,
        progress,
        p4kx_export::CatalogPublicationMode::Normal,
    )
}

fn build_with_mode(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
    progress: Option<&(dyn Fn(CatalogProgress) + Sync)>,
    publication_mode: p4kx_export::CatalogPublicationMode,
) -> Result<BuiltCatalog, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive(dir.path(), forge)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions {
        publication_mode,
        filter,
        progress,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    Ok(build_catalog(&archive, &options)?)
}

/// Returns the blueprint with the GUID `guid`.
pub(crate) fn blueprint(catalog: &Value, guid: u128) -> Result<&Value, Box<dyn Error>> {
    let wanted = text(guid);
    catalog["blueprints"]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["guid"] == wanted.as_str()))
        .ok_or_else(|| format!("blueprint {wanted}").into())
}
