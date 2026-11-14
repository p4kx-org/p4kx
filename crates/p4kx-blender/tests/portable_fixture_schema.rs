// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Validates scene manifests written by the synthetic native CLI producer.

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

#[test]
#[ignore = "set P4KX_BLENDER_FIXTURES to the native synthetic CLI output"]
fn native_cli_packages_match_the_manifest_schema() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(
        std::env::var_os("P4KX_BLENDER_FIXTURES").ok_or("synthetic package root is required")?,
    );
    assert!(
        root.is_absolute(),
        "the synthetic package root must be absolute"
    );
    let schema = p4kx_blender::manifest_schema();
    let validator = jsonschema::validator_for(&schema).map_err(|error| error.to_string())?;
    for case in ["assembly", "livery"] {
        let manifest: Value =
            serde_json::from_slice(&fs::read(root.join(case).join("scene.json"))?)?;
        let errors: Vec<_> = validator
            .iter_errors(&manifest)
            .map(|error| error.to_string())
            .collect();
        assert!(
            errors.is_empty(),
            "{case} manifest differs from its schema: {errors:?}"
        );
    }
    Ok(())
}
