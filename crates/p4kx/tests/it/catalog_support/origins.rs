// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_extraction::{Archive, Language};

/// Checks the original encrypted key source and plain locale source of a catalog fixture.
pub(crate) fn assert_input_encryption(
    path: &Path,
    language: Language,
) -> Result<(), Box<dyn Error>> {
    let archive = Archive::open(path)?;
    for (path, encrypted) in [
        (r"Data\Game2.dcb", true),
        (language.global_ini_path(), false),
    ] {
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == path)
            .ok_or("the original fixture Entry exists")?;
        assert_eq!(entry.is_encrypted(), encrypted, "original Entry: {path}");
    }
    Ok(())
}
