// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use serde_json::Value;

use super::TestResult;
use super::reference_log_report::Report;
use super::reference_manifest::Manifest;
use super::reference_prepare::Plan;

fn schemas() -> Result<Vec<(&'static str, Value)>, serde_json::Error> {
    Ok(vec![
        (
            "p4kx-reference.schema.json",
            serde_json::to_value(schemars::schema_for!(Manifest))?,
        ),
        (
            "p4kx-reference-plan.schema.json",
            serde_json::to_value(schemars::schema_for!(Plan))?,
        ),
        (
            "p4kx-reference-errors.schema.json",
            serde_json::to_value(schemars::schema_for!(Report))?,
        ),
    ])
}

fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas")
}

#[test]
fn reference_manifest_schemas_match_their_versioned_dtos() -> TestResult {
    for (name, expected) in schemas()? {
        let bytes = std::fs::read(folder().join(name))?;
        let actual: Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            actual, expected,
            "checked-in reference schema matches its versioned DTO"
        );
    }
    Ok(())
}

#[test]
#[ignore = "manual reference schema generation: set P4KX_WRITE_REFERENCE_SCHEMAS=1"]
fn write_reference_schemas() -> TestResult {
    if std::env::var("P4KX_WRITE_REFERENCE_SCHEMAS").as_deref() != Ok("1") {
        super::report("skipped: reference schema generation is not requested");
        return Ok(());
    }
    for (name, schema) in schemas()? {
        std::fs::write(folder().join(name), serde_json::to_vec_pretty(&schema)?)?;
    }
    Ok(())
}
