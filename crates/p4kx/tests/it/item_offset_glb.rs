// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{
    CheckedGlb, MOUNT_BONE, MOUNT_COURIER, TestEntry, TestModel, TestNode, check_glb,
    mounted_item_entries, test_key,
};
use serde_json::Value;

use super::{TestResult, crch_parent, offset_entries, offset_model, write_archive};
use crate::armour_mount_origins::mount_archive;

#[test]
fn armour_glb_reports_unusable_item_offset_helpers_at_the_port_placement() -> TestResult {
    let directory = tempfile::tempdir()?;
    for (name, z_scale, kind) in [
        ("spare_locator", 1.0, "item_offset_helper_not_found"),
        ("satchel_loc", 0.0, "item_offset_helper_not_invertible"),
    ] {
        let case = directory.path().join(name);
        fs::create_dir(&case)?;
        let archive = mount_archive(&case, offset_armour_entries(name, z_scale)?, false)?;
        let out = case.join("armour.glb");
        let result = p4kx_testkit::p4kx()?
            .args(["--json", "armour", "glb"])
            .arg(archive)
            .args(["--loadout", MOUNT_COURIER, "--out"])
            .arg(&out)
            .output()?;
        assert!(result.status.success(), "{result:?}");
        let report: Value = serde_json::from_slice(&result.stdout)?;
        check_diagnostic(&report, kind, "satchel_loc")?;
        check_port_pose(&check_glb(&fs::read(out)?)?)?;
    }
    Ok(())
}

fn check_port_pose(glb: &CheckedGlb) -> TestResult {
    let part = glb
        .document
        .nodes()
        .find(|node| node.name() == Some(MOUNT_BONE))
        .and_then(|bone| bone.children().find(|node| node.name() == Some("satchel")))
        .ok_or("the worn piece remains under the holder bone")?;
    let (translation, _, _) = part.transform().decomposed();
    assert!(
        translation
            .into_iter()
            .zip([0.0, 0.6, 0.2])
            .all(|(actual, expected)| (actual - expected).abs() < 1e-5),
        "the rigid piece keeps the port translation: {translation:?}"
    );
    Ok(())
}

fn offset_armour_entries(name: &str, z_scale: f32) -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let mut entries = mounted_item_entries();
    let slot = entries
        .iter_mut()
        .find(|entry| entry.path().ends_with("\\satchel.cgf"))
        .ok_or("satchel model")?;
    let node = |name: &str, parent, scale, z, kind| TestNode {
        name: name.to_owned(),
        parent,
        local: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, scale, z],
        ],
        kind,
    };
    let model = TestModel::cube().with_nodes(vec![
        node("satchel", None, 1.0, 0.0, 0),
        node(name, Some(0), z_scale, 0.5, 3),
    ]);
    *slot = TestEntry::stored(slot.path().to_owned(), model.cgf());
    Ok(entries)
}

#[test]
fn entity_glb_reports_unusable_item_offset_helpers_at_the_port_placement() -> TestResult {
    let directory = tempfile::tempdir()?;
    for (node, invertible, kind) in [
        ("spare_socket", true, "item_offset_helper_not_found"),
        ("item_socket", false, "item_offset_helper_not_invertible"),
    ] {
        let bytes = offset_model(node, invertible);
        let (report, transform) = export_item_offset(directory.path(), &bytes, "item_socket")?;
        let (control, port) = export_item_offset(directory.path(), &bytes, "")?;
        check_diagnostic(&report, kind, "item_socket")?;
        assert_eq!(
            report.get("skipped"),
            control.get("skipped"),
            "a rejected item offset helper leaves the part included: {report}"
        );
        assert_eq!(transform, port, "{kind} keeps the authored port placement");
    }
    Ok(())
}

type EncodedTransform = [[u32; 4]; 4];

fn export_item_offset(
    folder: &Path,
    child: &[u8],
    helper: &str,
) -> Result<(Value, EncodedTransform), Box<dyn Error>> {
    let input = tempfile::tempdir_in(folder)?;
    let entries = offset_entries(crch_parent()?, child.to_vec(), helper);
    let archive = write_archive(input.path(), entries)?;
    let out = input.path().join("ship.glb");
    let result = p4kx_testkit::p4kx()?
        .args(["--json", "entity", "glb"])
        .arg(archive)
        .args(["TEST_Ship", "--key", &test_key().to_hex(), "--out"])
        .arg(&out)
        .output()?;
    assert!(result.status.success(), "{result:?}");
    let report = serde_json::from_slice(&result.stdout)?;
    let glb = check_glb(&fs::read(out)?)?;
    let part = glb
        .document
        .nodes()
        .find(|node| node.name() == Some("socket/TEST_Child"))
        .ok_or("the attachment stays in the GLB")?;
    Ok((
        report,
        part.transform().matrix().map(|row| row.map(f32::to_bits)),
    ))
}

fn check_diagnostic(report: &Value, kind: &str, helper: &str) -> TestResult {
    let warning = report
        .get("issues")
        .and_then(Value::as_array)
        .ok_or("assembly issues")?
        .iter()
        .find(|issue| issue.get("kind").and_then(Value::as_str) == Some(kind))
        .ok_or_else(|| format!("expected {kind}: {report}"))?;
    assert_eq!(
        warning.pointer("/skipped"),
        Some(&Value::Bool(false)),
        "{warning}"
    );
    assert!(
        warning
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| reason.contains(helper)),
        "the diagnostic names the authored locator: {warning}"
    );
    Ok(())
}
