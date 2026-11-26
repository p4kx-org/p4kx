// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_export::{CatalogError, ShipBaseError};
use p4kx_extraction::{ArchiveWatch, DataForgeLoadError};

use super::{ARCHIVE_CHANGED, DATAFORGE_ERROR, PARTIAL_FAILURE, code_for};

#[test]
fn changed_input_keeps_exit_code_through_cli_wrappers() -> Result<(), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("input");
    fs::write(&path, b"initial")?;
    let watch = ArchiveWatch::of_file(&path)?;
    fs::write(&path, b"changed length")?;
    let changed = watch.check().expect_err("fixture size changed");

    let standalone = DataForgeLoadError::ArchiveChanged(changed.clone());
    assert_eq!(code_for(&standalone.into()), ARCHIVE_CHANGED, "dcb");
    let catalog = CatalogError::ArchiveChanged(changed.clone());
    assert_eq!(code_for(&catalog.into()), ARCHIVE_CHANGED, "catalog");
    let ships = ShipBaseError::from(CatalogError::ArchiveChanged(changed));
    assert_eq!(code_for(&ships.into()), ARCHIVE_CHANGED, "ships");

    assert_eq!(
        code_for(&DataForgeLoadError::Missing.into()),
        DATAFORGE_ERROR,
        "a missing DataForge"
    );
    assert_eq!(
        code_for(&ShipBaseError::Cancelled.into()),
        PARTIAL_FAILURE,
        "a cancelled ship base export"
    );
    Ok(())
}
