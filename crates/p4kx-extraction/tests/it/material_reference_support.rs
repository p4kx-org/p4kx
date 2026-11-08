// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, material_reference_entries};

pub(super) const TARGET: &str = "Data/Objects/Test/paint.mtl";
pub(super) type TestResult = Result<(), Box<dyn Error>>;

pub(super) fn archive(
    folder: &Path,
    reference: &str,
    target: Option<&str>,
    extra: impl IntoIterator<Item = TestEntry>,
) -> Result<Archive, Box<dyn Error>> {
    let path = folder.join("fixture.p4k");
    material_reference_entries(reference, target)
        .into_iter()
        .chain(extra)
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    Ok(Archive::open(&path)?)
}
