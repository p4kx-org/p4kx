// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use crate::{
    Archive, InputScope, SourceCoordinateOmission, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Original {
    archive: Archive,
    _directory: tempfile::TempDir,
}

struct Projection {
    scope: SourceScope,
    used: SourceUse,
    cell: SourcedValue<String>,
}

#[test]
fn unavailable_coordinates_keep_genuine_input_witnesses_and_incomplete_coverage() -> TestResult {
    for encrypted in [false, true] {
        let original = original(encrypted)?;
        let sources = SourceUses::new();
        let projection = project(&original.archive, &sources)?;
        assert_projection(&projection, encrypted)?;
        assert_control(&projection.scope, &sources)?;
    }
    Ok(())
}

fn original(encrypted: bool) -> TestResult<Original> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let entry = TestEntry::stored("actual.txt", "original");
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok(Original {
        archive,
        _directory: directory,
    })
}

fn project(archive: &Archive, sources: &SourceUses) -> TestResult<Projection> {
    use std::io::Read as _;
    let entry = archive.entries().first().ok_or("original admitted Entry")?;
    let original = archive
        .observed_entry(entry)
        .ok_or("original input proof")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    let value = std::str::from_utf8(&bytes)?.to_ascii_uppercase();
    assert_eq!(
        value, "ORIGINAL",
        "the value comes from the actual original read"
    );
    let scope = sources.observe(SourceInput::ArchiveView(original), InputScope::Payload)?;
    let used = scope.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::Unavailable,
    )?;
    let cell = sources.value(
        SourceValue::Present(value),
        std::slice::from_ref(&used),
        true,
    );
    Ok(Projection { scope, used, cell })
}

fn assert_projection(projection: &Projection, encrypted: bool) -> TestResult {
    let origin = projection.cell.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        if encrypted { Some(true) } else { None },
        "known encrypted witness survives missing original coordinates"
    );
    assert!(
        !origin.source_set_complete(),
        "unavailable coordinates never certify complete coverage"
    );
    assert_eq!(
        origin.source_uses_refs(),
        std::slice::from_ref(projection.used.reference()),
        "the real original use is retained"
    );
    assert_location(&serde_json::to_value(
        projection.used.description().location(),
    )?)
}

fn assert_control(scope: &SourceScope, sources: &SourceUses) -> TestResult {
    let attempted = scope.at_opaque(
        SourceUseRole::Attempted,
        SourceCoordinateOmission::Unavailable,
    )?;
    let control = sources.value(SourceValue::Present("control"), &[attempted], true);
    assert_eq!(
        control.origin().contains_encrypted_source(),
        Some(false),
        "attempted input cannot taint an independent control"
    );
    assert!(
        control.origin().source_uses_refs().is_empty(),
        "attempts are not contributing references"
    );
    Ok(())
}

fn assert_location(location: &Value) -> TestResult {
    assert_eq!(
        location,
        &json!({"type":"opaque", "reason":"unavailable"}),
        "the producer uses the closed protocol spelling"
    );
    let schema = serde_json::to_value(schemars::schema_for!(SourceLocation))?;
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(location),
        "the actual producer validates against the shared schema"
    );
    let mut unknown = location.clone();
    *unknown.get_mut("reason").ok_or("reason mutation target")? = json!("invented");
    assert!(
        !validator.is_valid(&unknown),
        "unrecognized omission reasons are not schema-admitted"
    );
    Ok(())
}
