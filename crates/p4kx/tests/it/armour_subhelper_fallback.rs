// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    CheckedGlb, MOUNT_COURIER, MOUNT_REPLACEMENT_WITHOUT_ROOT, SATCHEL_HELPER_Z, check_glb,
    mounted_unresolved_helper_entries, mounted_unresolved_helper_without_root_entries,
    mounted_unresolved_replacement_entries, test_key,
};
use serde_json::Value;

use super::{TestResult, check_artifact_binding, contributors, exact_mount_paths, mount_archive};

const PORT: &str = "Components[1].Ports[0].AttachmentImplementation.Helper";
const REPLACEMENT_PORT: &str = "Components[0].Ports[0].AttachmentImplementation.Helper";
const ORIGINAL: &str = "Mount_Carrier";

#[test]
fn unresolved_nested_worn_helper_warns_and_exports_only_the_root_transform() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = mount_archive(directory.path(), mounted_unresolved_helper_entries(), true)?;
    let holder = record_index(&archive, ORIGINAL)?;
    let out = directory.path().join("armour.glb");
    let report = export(&archive, &out, &[])?;
    check_diagnostic(&report, holder, PORT)?;
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
    let holder = record_index(&archive, ORIGINAL)?;
    let out = directory.path().join("armour.glb");
    let report = export(&archive, &out, &[])?;
    check_diagnostic(&report, holder, PORT)?;
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

#[test]
fn replacement_without_root_keeps_only_its_own_helper_diagnostic() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = mount_archive(
        directory.path(),
        mounted_unresolved_replacement_entries(),
        true,
    )?;
    let original = record_index(&archive, ORIGINAL)?;
    let replacement = record_index(&archive, MOUNT_REPLACEMENT_WITHOUT_ROOT)?;
    assert_ne!(
        original, replacement,
        "the holders have distinct physical Records"
    );
    let out = directory.path().join("armour.glb");
    let report = export(&archive, &out, &[MOUNT_REPLACEMENT_WITHOUT_ROOT])?;
    check_diagnostic(&report, replacement, REPLACEMENT_PORT)?;
    let attachment = record_index(&archive, "Mount_Satchel")?;
    check_mount_refusal_order(&report, attachment)?;

    let glb = check_glb(&fs::read(out)?)?;
    assert!(
        glb.document
            .nodes()
            .all(|node| node.name() != Some("satchel")),
        "the rejected attachment uses neither the old mount nor the scene origin",
    );
    Ok(())
}

fn check_mount_refusal_order(report: &Value, attachment: u32) -> TestResult {
    let issues = report
        .get("issues")
        .and_then(Value::as_array)
        .ok_or("armour issues")?;
    let mut relevant = issues.iter().filter(|issue| {
        matches!(
            issue.get("kind").and_then(Value::as_str),
            Some("port_helper_unresolved" | "worn_mount_unresolved")
        )
    });
    let (Some(helper), Some(refusal), None) = (relevant.next(), relevant.next(), relevant.next())
    else {
        return Err("one physical helper warning and one mount refusal are required".into());
    };
    assert_eq!(
        helper.get("kind").and_then(Value::as_str),
        Some("port_helper_unresolved"),
        "show the original helper failure first"
    );
    assert!(
        helper
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| {
                reason.contains(&format!("for attached Record {attachment}:"))
            }),
        "the helper diagnostic retains the physical attached item: {helper}",
    );
    assert_eq!(
        refusal.get("kind").and_then(Value::as_str),
        Some("worn_mount_unresolved"),
        "retain the mount refusal"
    );
    assert_eq!(
        refusal.get("skipped").and_then(Value::as_bool),
        Some(true),
        "the mount refusal omits the rigid piece"
    );
    Ok(())
}

fn export(archive: &Path, out: &Path, pieces: &[&str]) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
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
        .arg(out);
    for piece in pieces {
        command.args(["--piece", piece]);
    }
    let result = command.output()?;
    assert!(
        result.status.success(),
        "the remaining worn models are drawable: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(serde_json::from_slice(&result.stdout)?)
}

fn record_index(path: &Path, class: &str) -> Result<u32, Box<dyn Error>> {
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let input = DataForgeEntry::read(&archive)?;
    let forge = input.open()?;
    let record_path = format!("entities/scitem/{class}.xml");
    let mut records = forge.records_at_path(&record_path);
    let record = records.next().ok_or("the original selected Record")?;
    assert!(
        records.next().is_none(),
        "the fixture path has one physical Record"
    );
    Ok(record.index())
}

fn check_diagnostic(report: &Value, holder: u32, port: &str) -> TestResult {
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
            "Record {holder} {port}.SubHelpers[1].SubHelpers[0]"
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
            .zip([0.0, 0.5 + 0.2 - SATCHEL_HELPER_Z, -0.1])
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
