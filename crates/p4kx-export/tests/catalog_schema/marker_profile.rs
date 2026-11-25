// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::TestResult;

#[test]
fn marker_schema_requires_distinct_wire_and_preserves_game_text() -> TestResult {
    let full = p4kx_export::catalog_schema();
    let marker = p4kx_export::catalog_marker_schema();
    assert_eq!(
        marker["properties"]["format"]["const"], "p4kx-catalog-markers",
        "format id"
    );
    assert_eq!(
        marker["properties"]["formatVersion"]["const"], 1,
        "format version"
    );
    assert_eq!(
        marker["properties"]["catalog"]["properties"]["schemaVersion"]["const"], 3,
        "catalog version"
    );

    for name in ["SourcedValueOfboolean", "VariantDecision"] {
        let scoped = json!({
            "$defs": marker["$defs"],
            "$ref": format!("#/$defs/{name}"),
        });
        let validator = jsonschema::validator_for(&scoped)?;
        let value = json!({"state":"absent","value":null,"containsEncryptedSource":true});
        assert!(
            validator.is_valid(&value),
            "{name} allows a positive marker without proofs"
        );
        let mut with_proof = value;
        with_proof["sourceUsesRefs"] = json!([]);
        assert!(
            !validator.is_valid(&with_proof),
            "{name} rejects proof entries"
        );
    }
    let annotation = "/$defs/GameTextRef/x-gameText";
    let game_text = full
        .pointer(annotation)
        .ok_or("Game Text schema annotation")?;
    assert_eq!(
        marker.pointer(annotation),
        Some(game_text),
        "Game Text protection"
    );
    Ok(())
}

#[test]
fn marker_schema_projects_flattened_origins_inside_mission_binding_alternatives() -> TestResult {
    let marker = p4kx_export::catalog_marker_schema();
    let full = p4kx_export::catalog_schema();
    for (name, mut absent, present) in [
        (
            "DeclarationBinding",
            json!({"state":"absent"}),
            json!({"state":"present","field":{"type":"root","rootFieldIndex":"2"}}),
        ),
        (
            "FlowFieldBinding",
            json!({"type":"absent"}),
            json!({"type":"field","fieldIndex":"2"}),
        ),
    ] {
        let reference = format!("#/$defs/{name}");
        let marker_scope = json!({"$defs":marker["$defs"],"$ref":reference});
        let marker_validator = jsonschema::validator_for(&marker_scope)?;
        let full_scope = json!({"$defs":full["$defs"],"$ref":reference});
        let full_validator = jsonschema::validator_for(&full_scope)?;
        assert!(
            marker_validator.is_valid(&present),
            "{name} preserves its present binding"
        );
        assert!(
            full_validator.is_valid(&present),
            "{name} keeps the full present binding"
        );
        assert!(
            !marker_validator.is_valid(&absent),
            "{name} requires the encryption marker"
        );
        for encryption in [json!(true), json!(false), json!(null)] {
            absent["containsEncryptedSource"] = encryption;
            assert!(
                marker_validator.is_valid(&absent),
                "{name} accepts its absent marker"
            );
            assert!(
                !full_validator.is_valid(&absent),
                "{name} full requires evidence"
            );
        }
        absent["containsEncryptedSource"] = json!(false);
        absent["sourceSetComplete"] = json!(true);
        absent["sourceUsesRefs"] = json!([]);
        assert!(
            !marker_validator.is_valid(&absent),
            "{name} rejects full proofs in marker output"
        );
        assert!(
            full_validator.is_valid(&absent),
            "{name} keeps full proof validation"
        );
    }
    Ok(())
}
