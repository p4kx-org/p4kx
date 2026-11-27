// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, LoadoutError, LoadoutFileError, LoadoutFiles, VehicleImplementation,
};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry};
use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const PRIMARY: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Primary.xml";
const BASE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Mixed.xml";
const WANTED: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/Wanted.xml";
const FALLBACK: &str =
    "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/Primary_Variant.xml";
const PATCH: &[u8] = b"<Patch/>";
const XML: &[u8] = br#"<Vehicle><Modifications><Modification name="Variant" patchFile="Modifications/Wanted.xml"/></Modifications></Vehicle>"#;

fn input(modification: &str) -> TestResult<Fixture> {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(
                0x5742,
                "Mixed",
                "libs/foundry/records/entities/spaceships/mixed.xml",
            )
            .attach_type("NOITEM_Vehicle")
            .vehicle(PRIMARY, modification)
            .record(),
        )
        .build();
    Fixture::open(
        &bytes,
        Input::Plain,
        vec![
            TestEntry::stored(format!("Data/{BASE}"), XML).encrypted(),
            TestEntry::stored(format!("Data/{FALLBACK}"), PATCH),
        ],
    )
}

fn original(source: &Fixture, modification: Option<&str>) -> TestResult {
    let forge = source.entry.open()?;
    let record = forge.record(0).ok_or("the genuine source Record")?;
    assert_eq!(
        record.type_name()?,
        "EntityClassDefinition",
        "the original source type"
    );
    assert_eq!(
        record.tree()?.error_count(),
        0,
        "the original source tree is fully readable"
    );
    let files = ArchiveFiles::new(&source.archive);
    assert!(
        matches!(files.read(PRIMARY), Err(LoadoutFileError::NotFound)),
        "the primary definition really is missing"
    );
    assert_eq!(
        files.read(BASE)?,
        XML,
        "the actual class fallback supplies these exact decrypted XML bytes"
    );
    assert!(
        matches!(files.read(WANTED), Err(LoadoutFileError::NotFound)),
        "the explicit patch really is missing"
    );
    assert_eq!(
        files.read(FALLBACK)?,
        PATCH,
        "the actual stem fallback is readable"
    );
    let vehicle = VehicleImplementation::read(&files, "Mixed", PRIMARY, modification)?;
    assert_eq!(
        vehicle.path(),
        BASE,
        "the real implementation selects the successful class fallback"
    );
    if modification.is_some() {
        assert_eq!(
            vehicle.patch_path(),
            Some(FALLBACK),
            "the real implementation selects the successful patch fallback"
        );
        assert!(
            vehicle.issues().iter().any(|issue| matches!(issue,
            LoadoutError::MissingVehicleImplementation { path } if path.value() == WANTED)),
            "the actual missing explicit patch remains an issue beside the successful fallback"
        );
    } else {
        assert!(
            vehicle.issues().is_empty(),
            "a successful definition fallback creates no missing-definition issue"
        );
    }
    Ok(())
}

#[test]
fn a_successful_definition_fallback_does_not_become_a_missing_body_error() -> TestResult {
    let source = input("")?;
    original(&source, None)?;
    let (_, catalog) = source.diagnostic()?;
    assert_eq!(
        check::rows(&catalog, "/ships")?.len(),
        1,
        "the genuine successful fallback still publishes its source row"
    );
    assert!(
        diagnostics(&catalog)?.is_empty(),
        "the actual missing primary does not fabricate a MissingVehicle issue"
    );
    check::closure(&catalog)
}

#[test]
fn a_missing_explicit_patch_keeps_its_body_beside_successful_definition_and_patch_io() -> TestResult
{
    let source = input("Variant")?;
    original(&source, Some("Variant"))?;
    let (_, catalog) = source.diagnostic()?;
    assert_eq!(
        check::rows(&catalog, "/ships")?.len(),
        1,
        "successful fallback IO does not refuse the genuine partial source row"
    );
    let rows = diagnostics(&catalog)?;
    assert_eq!(
        rows.len(),
        1,
        "one actual missing explicit patch diagnostic"
    );
    let diagnostic = rows.first().ok_or("the actual explicit patch issue")?;
    assert_eq!(
        diagnostic.get("count"),
        Some(&json!(1)),
        "one exact source issue"
    );
    let examples = check::rows(diagnostic, "/examples")?;
    assert_eq!(examples.len(), 1, "one original issue example");
    let example = examples
        .first()
        .ok_or("the original explicit issue example")?;
    let field = check::present_text(&catalog, example, "field", &json!(WANTED), Some(true))?;
    assert!(
        check::uses(&catalog, field)?
            .iter()
            .any(|used| used.get("role") == Some(&json!("payload"))
                && used.pointer("/location/namespace") == Some(&json!("xml"))
                && used.pointer("/location/field") == Some(&json!("@patchFile"))
                && check::source(&catalog, used)
                    .is_ok_and(|source| source.pointer("/path/value")
                        == Some(&json!(format!("Data/{BASE}"))))),
        "the original encrypted XML patchFile datum supplies the unchanged missing identity"
    );
    let kind = check::present_text(
        &catalog,
        example,
        "sourceType",
        &json!("EntityClassDefinition"),
        Some(false),
    )?;
    check::header(&catalog, kind, 0, "sourceType", "selector", "archiveEntry")?;
    assert_eq!(
        example.pointer("/targetGuid/state"),
        Some(&json!("absent")),
        "no target GUID is fabricated"
    );
    check::closure(&catalog)
}

fn diagnostics(catalog: &Value) -> TestResult<Vec<&Value>> {
    Ok(check::rows(catalog, "/exportHealth/diagnostics")?
        .iter()
        .filter(|row| {
            row.get("code") == Some(&json!("danglingReference"))
                && row.get("warningCode") == Some(&json!("vehicle_implementation"))
                && row.get("kind") == Some(&json!("loadouts"))
        })
        .collect())
}

#[path = "health_loadout_vehicle_name_tests.rs"]
mod selected_name;
