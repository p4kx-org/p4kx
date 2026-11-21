// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{TestEntry, hologram_suppression_entries, test_key};
use serde_json::{Value, json};

use super::{TestResult, hologram, receipt, skip, source_uses, write_archive};

fn fixture(encrypt_suppressed: bool) -> Vec<TestEntry> {
    hologram_suppression_entries()
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            if path.ends_with("cube.mtl") || (encrypt_suppressed && path.ends_with("ship.mtl")) {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect()
}

#[test]
fn only_material_decisions_that_suppressed_geometry_contribute_to_the_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let key = test_key().to_hex();
    let out = dir.path().join("ship.png");
    let mut baseline = None;
    for encrypted in [true, false] {
        let input = tempfile::tempdir_in(dir.path())?;
        let archive = write_archive(input.path(), fixture(encrypted))?;
        let run = hologram(&archive, &out, &["--size", "96", "--key", &key], true)?;
        if skip(
            "only_material_decisions_that_suppressed_geometry_contribute_to_the_png",
            &run,
        ) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&out, &report)?;
        check_uses(&rows, &report)?;
        let marker = json!(encrypted.then_some(true));
        assert_eq!(
            report["artifactOrigin"]["containsEncryptedSource"], marker,
            "the encrypted library only suppresses geometry"
        );
        assert_eq!(
            report["artifactOrigin"]["sourceSetComplete"], false,
            "known suppression is partial evidence"
        );
        let bytes = fs::read(&out)?;
        if let Some(previous) = baseline {
            assert_eq!(
                bytes, previous,
                "identical render with only the original encryption changed"
            );
        }
        baseline = Some(bytes);
    }
    Ok(())
}

#[test]
fn a_suppressed_range_of_a_culled_piece_is_not_a_picture_input() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path(), fixture(true))?;
    let key = test_key().to_hex();
    let out = dir.path().join("ship.png");
    let run = hologram(&archive, &out, &["--size", "1", "--key", &key], true)?;
    if skip(
        "a_suppressed_range_of_a_culled_piece_is_not_a_picture_input",
        &run,
    ) {
        return Ok(());
    }
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    let rows = receipt(&out, &report)?;
    let (_, _, pixels) = super::super::pixels(&out)?;
    assert_eq!(
        pixels,
        [[0; 4]],
        "the admitted pieces are below the pixel threshold"
    );
    let gun = source_uses(&rows, &report, "gun_paint.mtl");
    assert!(
        gun.iter()
            .all(|row| row["location"]["field"] != "decalMaterial"),
        "the culled pieces cannot contribute their omitted range"
    );
    let hull = source_uses(&rows, &report, "ship.mtl");
    material(&hull, "hiddenMaterial", "1")?;
    Ok(())
}

fn check_uses(rows: &[Value], report: &Value) -> TestResult {
    let whole = source_uses(rows, report, "ship.mtl");
    let partial = source_uses(rows, report, "gun_paint.mtl");
    assert_eq!(whole.len(), 2, "two distinct suppressed library slots");
    let hidden = material(&whole, "hiddenMaterial", "1")?;
    let removed = material(&whole, "decalMaterial", "2")?;
    let ranged = material(&partial, "decalMaterial", "1")?;
    material(&partial, "selectedMaterial", "2")?;
    assert_eq!(
        source_uses(rows, report, "cube.mtl"),
        [] as [&Value; 0],
        "overridden library"
    );
    for path in ["/drawn_parts", "/triangles"] {
        let refs = report["fieldOrigins"]
            .get(path)
            .and_then(|field| field.get("sourceUsesRefs"))
            .and_then(Value::as_array)
            .ok_or("scene contributor references")?;
        assert!(
            refs.contains(hidden) && refs.contains(removed),
            "the omitted parts affect {path}"
        );
        assert!(
            !refs.contains(ranged),
            "only the picture uses a single omitted range"
        );
    }
    Ok(())
}

fn material<'a>(
    uses: &[&'a Value],
    field: &str,
    slot: &str,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let original = uses
        .iter()
        .find(|row| {
            row["role"] == "selector"
                && row["location"]
                    == json!({
                        "type": "subresource",
                        "namespace": "materialLibrary",
                        "indices": [slot],
                        "field": field,
                    })
        })
        .ok_or_else(|| format!("missing material slot {slot} for {field}"))?;
    original
        .get("id")
        .ok_or_else(|| "missing source use reference".into())
}
