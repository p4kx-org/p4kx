// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{MOUNT_BONE, MOUNT_COURIER, check_glb, mounted_item_entries, test_key};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

fn exact_mount_paths(bytes: &[u8]) -> Result<(String, String), Box<dyn Error>> {
    let glb = check_glb(bytes)?;
    let bone = glb
        .document
        .nodes()
        .find(|node| node.name() == Some(MOUNT_BONE))
        .ok_or("parent bone")?;
    let (ordinal, child) = bone
        .children()
        .enumerate()
        .find(|(_, node)| node.name() == Some("satchel"))
        .ok_or("mounted satchel")?;
    Ok((
        format!("/nodes/{}/children/{ordinal}", bone.index()),
        format!("/nodes/{}/translation", child.index()),
    ))
}

fn contributors<'a>(
    rows: &'a [Value],
    path: &str,
    encrypted: Option<bool>,
) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let field = rows
        .iter()
        .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(path))
        .ok_or("exact GLB address")?;
    assert_eq!(
        field.pointer("/address/type"),
        Some(&Value::from("glbJsonField")),
        "{path} addresses a field in the GLB JSON chunk"
    );
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&serde_json::to_value(encrypted)?),
        "{path} reports the measured encryption state"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&Value::Bool(false)),
        "{path} retains partial source coverage"
    );
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?;
    Ok(rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|source| source.get("id").is_some_and(|id| refs.contains(id)))
        .collect())
}

fn check_artifact_binding(bytes: &[u8], rows: &[Value]) -> TestResult {
    let header = rows.first().ok_or("receipt header")?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(bytes)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    assert_eq!(
        header.pointer("/binding/path/value"),
        Some(&Value::from("armour.glb")),
        "the receipt binds the artifact basename"
    );
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "the receipt digest covers the committed GLB bytes"
    );
    assert_eq!(
        header.pointer("/binding/byteLength"),
        digest.get("byteLength"),
        "the receipt length covers the committed GLB bytes"
    );
    assert_eq!(
        rows.last().and_then(|row| row.get("type")),
        Some(&Value::from("end")),
        "the receipt reaches its formal completion row"
    );
    Ok(())
}

fn check_proof(bytes: &[u8], text: &str, encrypted: Option<bool>) -> TestResult {
    let rows = text
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    check_artifact_binding(bytes, &rows)?;
    let (parent_path, translation_path) = exact_mount_paths(bytes)?;
    let parent = contributors(&rows, &parent_path, encrypted)?;
    let pose = contributors(&rows, &translation_path, encrypted)?;
    let has = |sources: &[&Value], leaf: &str, role: &str| {
        sources.iter().any(|source| {
            source.get("role").and_then(Value::as_str) == Some(role)
                && source
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    .is_some_and(|field| field.ends_with(leaf))
        })
    };
    let helper = "].Ports[0].AttachmentImplementation.Helper.Helper.Name";
    let position = "].Ports[0].AttachmentImplementation.Helper.Helper.Offset.Position";
    let locator = "].Ports[0].AttachmentImplementation.Helper.Helper.ItemOffsetHelperName";
    assert!(
        has(&parent, helper, "selector"),
        "the actual GLB parent slot names the authored helper selector"
    );
    assert!(
        !has(&parent, position, "conversion"),
        "the port position affects the pose only"
    );
    assert!(
        has(&pose, helper, "selector"),
        "the mount translation uses the authored helper selector"
    );
    assert!(
        has(&pose, position, "conversion"),
        "the mount translation uses the original port offset"
    );
    assert!(
        has(&pose, locator, "selector"),
        "the mount translation uses the item locator selector"
    );
    for (namespace, field) in [
        ("modelNodePlacementBytes", "localTransform"),
        ("modelBonePlacementBytes", "relativePose"),
    ] {
        assert!(
            pose.iter().any(|source| {
                source
                    .pointer("/location/namespace")
                    .and_then(Value::as_str)
                    == Some(namespace)
                    && source.pointer("/location/field").and_then(Value::as_str) == Some(field)
            }),
            "the mount translation includes {namespace}.{field}"
        );
    }
    Ok(())
}

#[test]
fn mounted_armour_glb_receipt_binds_physical_port_and_applied_model_fields() -> TestResult {
    let directory = tempfile::tempdir()?;
    let out = directory.path().join("armour.glb");
    let receipt = out.with_file_name("armour.glb.p4kx-origin.jsonl");
    let mut prior = None;
    for (label, encrypted) in [("encrypted", Some(true)), ("plain", None)] {
        let case = directory.path().join(label);
        fs::create_dir(&case)?;
        let archive = super::chf_glb::archive(
            &case,
            mounted_item_entries()
                .into_iter()
                .map(|entry| {
                    let dataforge = Path::new(entry.path())
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("dcb"));
                    if encrypted.is_some() && dataforge {
                        entry.encrypted()
                    } else {
                        entry
                    }
                })
                .collect(),
        )?;
        let mut command = p4kx_testkit::p4kx()?;
        command
            .args(["armour", "glb"])
            .arg(&archive)
            .args(["--loadout", MOUNT_COURIER, "--out"])
            .arg(&out);
        if encrypted.is_some() {
            command.args(["--key", &test_key().to_hex()]);
        }
        let result = command.output()?;
        assert!(
            result.status.success(),
            "{label} fixture Armour GLB export failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = fs::read(&out)?;
        check_proof(&bytes, &fs::read_to_string(&receipt)?, encrypted)?;
        if let Some(prior) = &prior {
            assert_eq!(
                &bytes, prior,
                "changing archive encryption preserves the GLB bytes"
            );
        }
        prior = Some(bytes);
    }
    Ok(())
}
