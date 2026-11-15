// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::chunks::ivo;
use p4kx_media::MediaRecord;
use p4kx_testkit::TestChunkFile;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::oracle::Evidence;
use super::{TestResult, check_rows, replay};

#[test]
fn collision_oracle_reconciles_every_disposition_and_exact_geometry() -> TestResult {
    let original = fixture()?;
    let expected = Evidence::read(&original)?;
    check_fixture(&expected);
    replay(&original, expected.partial(), |generation, rows| {
        let digest = Sha256::digest(&original);
        check_rows(generation, rows, &digest, &expected)?;
        for (namespace, selector, replacement) in [
            (
                "model-physics-slot",
                "/data/payload/blob/geometry/radius/bits",
                json!(0),
            ),
            ("model-physics-slot", "/data/payload/blob/marker", json!(12)),
            (
                "model-physics-vertex",
                "/data/payload/position/0/bits",
                json!(0),
            ),
            (
                "model-physics-triangle",
                "/data/payload/indices/2",
                json!(0),
            ),
        ] {
            let corrupt = altered(rows, namespace, selector, &replacement)?;
            assert!(
                check_rows(generation, &corrupt, &digest, &expected).is_err(),
                "exact replay comparison rejects altered {namespace} {selector}",
            );
        }
        Ok(())
    })
}

fn check_fixture(evidence: &Evidence<'_>) {
    let tally = evidence.tally();
    assert_eq!(
        tally.tables, 3,
        "valid, unsupported and malformed original tables"
    );
    assert!(
        evidence.partial(),
        "recognized malformed blobs are explicit partial rows"
    );
    assert_eq!(
        (tally.vertices, tally.triangles),
        (8, 4),
        "both mesh map variants are decoded"
    );
    for kind in [
        "box",
        "sphere",
        "cylinder",
        "capsule",
        "trimesh",
        "damage-mesh",
        "signed-distance-field",
        "malformed",
        "missing-payload",
        "unknown-kind",
        "unknown-marker",
    ] {
        assert!(
            tally.dispositions.contains_key(kind),
            "independent oracle exercises {kind}"
        );
    }
}

fn fixture() -> TestResult<Vec<u8>> {
    let measured = blob_table()?;
    let invalid = [0_u8; 10];
    Ok(TestChunkFile::ivo()
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x900, measured)
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x901, b"unsupported")
        .chunk(ivo::STAT_OBJ_PHYSICS.id(), 0x900, invalid)
        .bytes())
}

fn blob_table() -> TestResult<Vec<u8>> {
    let slots = [
        analytic(
            0,
            &[
                1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, -0.0, 2.3, 3.0, 4.0, 5.0, 6.0,
            ],
        ),
        analytic(4, &[1.001, 2.0, 3.0, 5.0]),
        analytic(5, &[1.0, 2.0, 3.0, 0.0, 0.0, 1.0, 0.25, 3.0]),
        analytic(6, &[6.0, 5.0, 4.0, 0.0, 1.0, 0.0, 0.75, 4.0]),
        mesh(false, 2),
        mesh(true, 3),
        mesh(true, 4),
        0xCAFE_BABE_u32.to_le_bytes().to_vec(),
        0xDEAD_BEB5_u32.to_le_bytes().to_vec(),
        9_u32.to_le_bytes().to_vec(),
        analytic(41, &[]),
        analytic(4, &[f32::NAN, 0.0, 0.0, 1.0]),
        analytic(6, &[1.0, 2.0, 3.0]),
    ];
    let mut table = vec![0; 80];
    table
        .get_mut(40..44)
        .ok_or("test header")?
        .copy_from_slice(&u32::try_from(slots.len() + 1)?.to_le_bytes());
    for (ordinal, slot) in slots.iter().enumerate() {
        table.extend(u32::try_from(slot.len())?.to_le_bytes());
        let phys = if ordinal < 2 {
            0x0002_FFFF
        } else {
            0x0002_000F
        };
        table.extend(u32::to_le_bytes(phys));
    }
    table.extend(96_u32.to_le_bytes());
    table.extend(0x0001_0009_u32.to_le_bytes());
    table.extend(slots.into_iter().flatten());
    Ok(table)
}

fn analytic(kind: u32, values: &[f32]) -> Vec<u8> {
    let mut blob = Vec::from(2_u32.to_le_bytes());
    blob.resize(68, 0);
    blob.extend(kind.to_le_bytes());
    for (ordinal, value) in values.iter().enumerate() {
        if kind == 0 && ordinal == 9 {
            blob.extend(1_u32.to_le_bytes());
        } else {
            blob.extend(value.to_le_bytes());
        }
    }
    blob
}

fn mesh(mapped: bool, last_index: u16) -> Vec<u8> {
    let mut blob = analytic(1, &[]);
    for word in [4_u32, 2, 3, 9, 2] {
        blob.extend(word.to_le_bytes());
    }
    blob.extend([u8::from(mapped), 0]);
    for auxiliary in [71_u16, 51]
        .into_iter()
        .chain(mapped.then_some(72))
        .chain(mapped.then_some(19))
    {
        blob.extend(auxiliary.to_le_bytes());
    }
    blob.extend([7, 11]);
    for value in [
        1.25_f32, -0.0, 3.0, 4.0, 5.2, 6.0, 7.0, 8.1, 9.0, 10.0, 11.0, 12.0,
    ] {
        blob.extend(value.to_le_bytes());
    }
    for index in [3_u16, 1, 2, 2, 0, last_index] {
        blob.extend(index.to_le_bytes());
    }
    blob.extend(b"opaque tree");
    blob
}

fn altered(rows: &str, namespace: &str, field: &str, replacement: &Value) -> TestResult<String> {
    let mut output = Vec::new();
    let mut changed = false;
    for line in rows.lines() {
        let row: MediaRecord = serde_json::from_str(line)?;
        let mut json: Value = serde_json::from_str(line)?;
        if !changed
            && row.subresource.namespace == namespace
            && let Some(value) = json.pointer_mut(field)
            && value != replacement
        {
            *value = replacement.clone();
            changed = true;
        }
        output.push(serde_json::to_string(&json)?);
    }
    assert!(
        changed,
        "mutation changed an actual {namespace} field {field}"
    );
    Ok(output.join("\n"))
}
