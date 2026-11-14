// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveFiles;
use p4kx_extraction::{Archive, DataForge, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::OriginalCatalogInput;
use super::test_support::TestResult;
use crate::catalog_origins::CatalogOrigins;
use crate::texts::TextSources;

/// Opens one genuine synthetic Archive and lends its checked input only within this callback.
pub(crate) fn with_input(
    bytes: &[u8],
    entries: Vec<TestEntry>,
    check: impl for<'input> FnOnce(
        &'input Archive,
        &'input ArchiveFiles<'input>,
        &'input DataForge<'input>,
        &'input OriginalCatalogInput<'input, 'input>,
    ) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("ship-cache-input.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored("Data\\Game2.dcb", bytes));
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    let files = ArchiveFiles::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let texts = TextSources::read(&archive)?;
    let origins = CatalogOrigins::new(&archive, &entry, &texts)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    check(&archive, &files, &forge, &input)
}
