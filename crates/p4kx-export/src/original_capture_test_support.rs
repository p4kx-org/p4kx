// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    Archive, DataForge, DataForgeEntry, DataForgeOrigin, InputScope, SourceInput,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::texts::TextSources;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn with_input(
    bytes: &[u8],
    check: impl FnOnce(&DataForge<'_>, &OriginalCatalogInput<'_, '_>) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("original-input.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let texts = TextSources::read(&archive)?;
    let origins = CatalogOrigins::new(&archive, &entry, &texts)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    check(&forge, &input)
}

pub(crate) fn with_shared_inventory_inputs(
    bytes: &[u8],
    check: impl FnOnce(
        &DataForge<'_>,
        &OriginalCatalogInput<'_, '_>,
        &DataForge<'_>,
        &OriginalCatalogInput<'_, '_>,
    ) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let first_path = folder.path().join("first.p4k");
    let second_path = folder.path().join("second.p4k");
    for path in [&first_path, &second_path] {
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data\\Game2.dcb", bytes))
            .write_to(path)?;
    }
    let first_archive = Archive::open(first_path)?;
    let second_archive = Archive::open(second_path)?;
    let first_entry = DataForgeEntry::read(&first_archive)?;
    let second_entry = DataForgeEntry::read(&second_archive)?;
    let first_forge = first_entry.open()?;
    let second_forge = second_entry.open()?;
    let first_texts = TextSources::read(&first_archive)?;
    let second_texts = TextSources::read(&second_archive)?;
    let first = CatalogOrigins::new(&first_archive, &first_entry, &first_texts)?;
    let mut second = CatalogOrigins::new(&second_archive, &second_entry, &second_texts)?;
    let DataForgeOrigin::Archive(origin) = second_entry.origin() else {
        return Err("second actual entry requires original Archive binding".into());
    };
    second.uses = first.uses.clone();
    second.dataforge = second
        .uses
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let first_input = OriginalCatalogInput::new(&first_forge, &first);
    let second_input = OriginalCatalogInput::new(&second_forge, &second);
    check(&first_forge, &first_input, &second_forge, &second_input)
}

/// Lends two genuine report inventories over one original Archive, Entry and Forge.
pub(crate) fn with_same_forge_reports(
    bytes: &[u8],
    check: impl for<'input> FnOnce(
        &'input DataForge<'input>,
        &'input OriginalCatalogInput<'input, 'input>,
        &'input OriginalCatalogInput<'input, 'input>,
    ) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("tag-relations.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let texts = TextSources::read(&archive)?;
    let first = CatalogOrigins::new(&archive, &entry, &texts)?;
    let second = CatalogOrigins::new(&archive, &entry, &texts)?;
    assert!(
        !first.uses.is_same_inventory(&second.uses),
        "two real reports over the identical Archive and Forge"
    );
    let first = OriginalCatalogInput::new(&forge, &first);
    let second = OriginalCatalogInput::new(&forge, &second);
    check(&forge, &first, &second)
}
