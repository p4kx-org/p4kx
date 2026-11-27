// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::TestResult;

pub(super) fn verify(manifest: &Value) -> TestResult {
    assert_eq!(
        manifest["version"], "1.25",
        "addressed Character clip fields"
    );
    let provenance = &manifest["scene_provenance"];
    let fields = provenance["fieldOrigins"]
        .as_object()
        .ok_or("clip fields")?;
    for (ordinal, stem) in ["nod", "blink"].into_iter().enumerate() {
        let address = format!("/meshes/0/rig/animations/{ordinal}");
        let name_address = address.clone() + "/name";
        let name = manifest
            .pointer(&name_address)
            .and_then(Value::as_str)
            .ok_or("written clip name")?;
        assert!(
            name.contains(stem),
            "the writer order uses the prepared clip slot"
        );
        let origin = fields.get(&name_address).ok_or("clip name origin")?;
        verify_name_source(provenance, origin, stem)?;
        let keys = fields.get(&(address + "/keys")).ok_or("key count origin")?;
        assert_eq!(keys["sourceSetComplete"], false, "derived key count");
    }
    let additive = fields
        .get("/meshes/0/rig/animations/1/additive")
        .ok_or("sampled additive origin")?;
    assert_eq!(
        additive["sourceSetComplete"], true,
        "sampled additive flag is generated"
    );
    assert_eq!(
        additive["sourceUsesRefs"],
        serde_json::json!([]),
        "no authored flag is copied"
    );
    let sampled_duration = fields
        .get("/meshes/0/rig/animations/1/duration_seconds")
        .ok_or("sampled duration origin")?;
    assert_eq!(
        sampled_duration["sourceSetComplete"], false,
        "sampled duration retains partial conversion evidence",
    );
    verify_name_source(provenance, sampled_duration, "blink")?;
    assert_eq!(
        sampled_duration["sourceUsesRefs"].as_array().map(Vec::len),
        Some(1),
        "sampled timing evidence excludes the pose rig",
    );
    Ok(())
}

fn verify_name_source(provenance: &Value, origin: &Value, stem: &str) -> TestResult {
    let sources = provenance
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .ok_or("clip sources")?;
    let uses = provenance
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("clip source uses")?;
    let references = origin["sourceUsesRefs"]
        .as_array()
        .ok_or("clip name uses")?;
    let filename = format!("{stem}.caf");
    assert!(
        uses.iter().any(|usage| references.contains(&usage["id"])
            && sources.iter().any(|source| {
                source["id"] == usage["source"]
                    && source
                        .pointer("/path/value")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value.ends_with(&filename))
            })),
        "the {stem} clip resolves to its own physical CAF",
    );
    Ok(())
}
