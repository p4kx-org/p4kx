// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::BodyVariant;
use p4kx_blender::write_player_source_bundle;
use p4kx_extraction::CancelToken;
use serde_json::{Value, json};

use super::TestResult;

#[test]
fn actor_geometry_preserves_competing_body_hints_and_original_positions() -> TestResult {
    let forge = super::forge(|actor| {
        let male = BodyVariant::Male.skeleton();
        let female = BodyVariant::Female.skeleton();
        actor
            .geometry("Objects/Synthetic/main.cdf", None)
            .geometry_variant("Male", male, None)
            .geometry_variant("Female", male, None)
            .geometry_variant("Unrelated>Male Female", female, None)
            .geometry_variant("", "Objects/Synthetic/alternative.chr", None)
    });
    let motion = super::motion_option_assertions::entries("");
    let (_fixture, archive) = super::fixture_with_forge(motion, [], forge)?;
    let directory = tempfile::tempdir()?;
    let output = directory.path().join("bundle");
    write_player_source_bundle(
        &archive,
        &[BodyVariant::Female, BodyVariant::Male],
        &output,
        &CancelToken::new(),
    )?;
    let rows = fs::read_to_string(output.join("records.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let candidates: Vec<_> = rows
        .iter()
        .filter(|row| row["type"] == "actorRigCandidate")
        .collect();
    let [main, matching, conflict, intermediate, untagged, nested] = candidates.as_slice() else {
        return Err("all physical geometry nodes must be present".into());
    };
    assert_eq!(
        main.pointer("/value/subGeometry"),
        Some(&json!([])),
        "top-level position"
    );
    verify_repeated_match(matching, conflict);
    verify_nested(intermediate, untagged, nested);
    verify_unresolved_owner(&rows)?;
    Ok(())
}

fn verify_repeated_match(matching: &Value, conflict: &Value) {
    assert_eq!(
        matching.pointer("/value/modelPath/value"),
        conflict.pointer("/value/modelPath/value"),
        "same physical model selector spelling can appear twice"
    );
    assert_ne!(
        matching.pointer("/value/modelPath/sourceUsesRefs"),
        conflict.pointer("/value/modelPath/sourceUsesRefs"),
        "both original fields remain distinct"
    );
    assert_eq!(
        matching.pointer("/value/bodyEvidence/authoredTags/value"),
        Some(&json!(["male"])),
        "first original selector has its authored tag"
    );
    assert_eq!(
        conflict.pointer("/value/bodyEvidence/authoredTags/value"),
        Some(&json!(["female"])),
        "second original selector retains its conflicting tag"
    );
    assert_eq!(
        conflict.pointer("/value/bodyEvidence/standardSkeletonPath/value"),
        Some(&json!(["male"])),
        "path comparison is separate evidence"
    );
}

fn verify_nested(intermediate: &Value, untagged: &Value, nested: &Value) {
    assert_eq!(
        intermediate.pointer("/value/subGeometry"),
        Some(&json!([2])),
        "intermediate node preserved"
    );
    assert_eq!(
        untagged.pointer("/value/tags/value"),
        Some(&json!("")),
        "explicit empty original tag remains present"
    );
    assert_eq!(
        nested.pointer("/value/subGeometry"),
        Some(&json!([2, 0])),
        "exact nested slot is preserved"
    );
    assert_eq!(
        nested.pointer("/value/bodyEvidence/authoredTags/value"),
        Some(&json!(["male", "female"])),
        "all candidate variant tags independently retained"
    );
    assert_eq!(
        nested.pointer("/value/bodyEvidence/standardSkeletonPath/value"),
        Some(&json!(["female"])),
        "normalized original skeleton path"
    );
    assert_eq!(
        nested.pointer("/value/modelPath/containsEncryptedSource"),
        Some(&json!(true)),
        "model selector's actual DataForge is encrypted"
    );
    assert_eq!(
        nested.pointer("/value/tags/containsEncryptedSource"),
        Some(&json!(true)),
        "geometry tag source itself is encrypted"
    );
}

fn verify_unresolved_owner(rows: &[Value]) -> TestResult {
    let selected = rows
        .iter()
        .find(|row| row["type"] == "actorRigCandidates")
        .ok_or("actor source inventory")?;
    assert_eq!(selected["state"], json!("present"), "observed actor source");
    let motion = rows
        .iter()
        .find(|row| row["type"] == "motionControllerTopology")
        .ok_or("controller topology component")?;
    assert_eq!(
        motion.pointer("/value/value/status"),
        Some(&json!("unresolved")),
        "actor candidates alone do not establish controller ownership"
    );
    Ok(())
}
