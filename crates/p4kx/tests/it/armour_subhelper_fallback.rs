// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    CheckedGlb, MOUNT_COURIER, SATCHEL_HELPER_Z, check_glb, mounted_unresolved_helper_entries,
    mounted_unresolved_helper_without_root_entries, test_key,
};
use serde_json::Value;

use super::{TestResult, check_artifact_binding, contributors, exact_mount_paths, mount_archive};

const PORT: &str = "Components[1].Ports[0].AttachmentImplementation.Helper";

#[test]
fn unresolved_nested_worn_helper_warns_and_exports_only_the_root_transform() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = mount_archive(directory.path(), mounted_unresolved_helper_entries(), true)?;
    let holder = holder_record_index(&archive)?;
    let out = directory.path().join("armour.glb");
    let report = export(&archive, &out)?;
    check_diagnostic(&report, holder)?;
    let bytes = fs::read(&out)?;
    let glb = check_glb(&bytes)?;
    let rows = fs::read_to_string(out.with_file_name("armour.glb.p4kx-origin.jsonl"))?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    check_artifact_binding(&bytes, &rows)?;
    check_root_mount(&glb, &rows, holder)
}

#[test]
fn unresolved_worn_helper_without_root_keeps_its_original_diagnostic() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = mount_archive(
        directory.path(),
        mounted_unresolved_helper_without_root_entries(),
        true,
    )?;
    let holder = holder_record_index(&archive)?;
    let out = directory.path().join("armour.glb");
    let report = export(&archive, &out)?;
    check_diagnostic(&report, holder)?;
    let bytes = fs::read(&out)?;
    let glb = check_glb(&bytes)?;
    let satchel = glb
        .document
        .default_scene()
        .and_then(|scene| scene.nodes().find(|node| node.name() == Some("satchel")))
        .ok_or("the unmounted satchel is a scene root")?;
    assert!(
        satchel
            .transform()
            .decomposed()
            .0
            .into_iter()
            .all(|coordinate| coordinate.abs() < f32::EPSILON),
        "the scene-root piece keeps its origin"
    );
    assert!(
        !out.with_file_name("armour.glb.p4kx-origin.jsonl").exists(),
        "without a mount there is no mount provenance receipt"
    );
    Ok(())
}

fn export(archive: &Path, out: &Path) -> Result<Value, Box<dyn Error>> {
    let result = p4kx_testkit::p4kx()?
        .args(["--json", "armour", "glb"])
        .arg(archive)
        .args([
            "--key",
            &test_key().to_hex(),
            "--body",
            "male",
            "--loadout",
            MOUNT_COURIER,
            "--out",
        ])
        .arg(out)
        .output()?;
    assert!(
        result.status.success(),
        "the original wearable is still drawable: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn holder_record_index(path: &Path) -> Result<u32, Box<dyn Error>> {
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let input = DataForgeEntry::read(&archive)?;
    let forge = input.open()?;
    let mut records = forge.records_at_path("entities/scitem/Mount_Carrier.xml");
    let holder = records.next().ok_or("the original holder Record")?;
    assert!(
        records.next().is_none(),
        "the fixture has one physical holder"
    );
    Ok(holder.index())
}

fn check_diagnostic(report: &Value, holder: u32) -> TestResult {
    let mut warnings = report
        .get("issues")
        .and_then(Value::as_array)
        .ok_or("armour issues")?
        .iter()
        .filter(|issue| {
            issue.get("kind").and_then(Value::as_str) == Some("port_helper_unresolved")
        });
    let warning = warnings.next().ok_or("the unresolved helper warning")?;
    assert!(
        warnings.next().is_none(),
        "only the equipped port reports a warning"
    );
    assert_eq!(
        warning.get("skipped").and_then(Value::as_bool),
        Some(false),
        "the selector warning does not omit the rigid piece"
    );
    let reason = warning
        .get("reason")
        .and_then(Value::as_str)
        .ok_or("warning reason")?;
    assert!(
        reason.contains(&format!(
            "Record {holder} {PORT}.SubHelpers[1].SubHelpers[0]"
        )) && reason.contains("the selector has no proven evaluation rule"),
        "the warning addresses the unsupported original nested selector: {reason}"
    );
    Ok(())
}

fn check_root_mount(glb: &CheckedGlb, rows: &[Value], holder: u32) -> TestResult {
    let (parent_path, translation_path) = exact_mount_paths(glb)?;
    let satchel = glb
        .document
        .nodes()
        .find(|node| node.name() == Some("satchel"))
        .ok_or("the mounted rigid piece")?;
    let (translation, _, _) = satchel.transform().decomposed();
    assert!(
        translation
            .into_iter()
            .zip([0.0, 0.6, -SATCHEL_HELPER_Z + 0.2])
            .all(|(actual, expected)| (actual - expected).abs() < 1e-5),
        "the rigid piece retains the root offset and item alignment: {translation:?}"
    );
    let parent = contributors(rows, &parent_path, Some(true))?;
    let pose = contributors(rows, &translation_path, Some(true))?;
    let root_source = |sources: &[&Value], leaf, role| {
        let path = format!("{PORT}.Helper.{leaf}");
        sources.iter().any(|source| {
            source
                .pointer("/location/recordIndex")
                .and_then(Value::as_u64)
                == Some(u64::from(holder))
                && source.pointer("/location/field").and_then(Value::as_str) == Some(path.as_str())
                && source.get("role").and_then(Value::as_str) == Some(role)
        })
    };
    assert!(
        root_source(&parent, "Name", "selector"),
        "the bone child cites the actual root helper on the physical holder"
    );
    for (leaf, role) in [
        ("Offset.Position", "conversion"),
        ("Offset.Rotation", "conversion"),
        ("ItemOffsetHelperName", "selector"),
    ] {
        assert!(
            root_source(&pose, leaf, role),
            "the emitted translation cites the original root {leaf}"
        );
    }
    assert!(
        pose.iter().all(|source| {
            source.get("role").and_then(Value::as_str) != Some("conversion")
                || !source
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    .is_some_and(|field| field.contains(".SubHelpers["))
        }),
        "the unresolved nested transform is outside the used conversion inputs"
    );
    Ok(())
}
