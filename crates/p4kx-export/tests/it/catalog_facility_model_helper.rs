// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{TestResult, build, fixture, interior_rows, proof};

#[test]
fn catalog_uses_the_selected_model_helper_and_its_original_parent_route() -> TestResult {
    for (case, translation, consumes_offset) in [
        (fixture::Case::HelperFrame, [6.0, 9.0, 3.0], false),
        (fixture::Case::HelperAtOrigin, [11.0, 22.0, 33.0], true),
    ] {
        let (_folder, archive) = fixture::archive(case)?;
        let catalog = build(&archive)?;
        let rows = interior_rows(&catalog)?;
        let [valid, invalid] = rows.as_slice() else {
            return Err("two physical SOC facilities".into());
        };
        let source = proof::at(valid, "/source")?;
        let [x, y, z] = translation;
        let cell = proof::present(
            source,
            "shipFrameTransform",
            &json!([[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]]),
            Some(true),
        )?;
        let refs = proof::rows(cell, "/sourceUsesRefs")?;
        let encoded = refs.iter().filter_map(Value::as_str);
        assert_eq!(
            encoded
                .clone()
                .filter(|value| value.ends_with("localTransform")
                    && value.contains("modelNodePlacementBytes"))
                .count(),
            2,
            "the helper and its parent supply independent original matrix ranges"
        );
        assert_eq!(
            encoded
                .clone()
                .filter(|value| value.contains(":payload:s14:dataforgeValue")
                    && value.ends_with(":$value"))
                .count(),
            if consumes_offset { 6 } else { 0 },
            "the six authored offset payloads contribute only when Assembly applies the offset"
        );
        assert!(
            encoded
                .clone()
                .any(|value| value.contains("Geometry.Geometry.path")),
            "the original hull geometry field selects the physical model"
        );
        assert_original_model(&catalog, cell)?;
        let local = proof::cell(source, "localTransform", "present")?;
        assert!(
            local["containsEncryptedSource"].is_null(),
            "the original plain ZIP stream has no CIG encryption declaration"
        );
        proof::cell(
            proof::at(invalid, "/source")?,
            "shipFrameTransform",
            "invalid",
        )?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn catalog_reports_missing_and_ambiguous_helpers_without_using_the_offset_as_a_fallback()
-> TestResult {
    for (case, state) in [
        (fixture::Case::MissingHelper, "absent"),
        (fixture::Case::AmbiguousHelper, "invalid"),
    ] {
        let (_folder, archive) = fixture::archive(case)?;
        let catalog = build(&archive)?;
        let rows = interior_rows(&catalog)?;
        let row = rows
            .first()
            .ok_or("original facility survives missing or ambiguous helper")?;
        let source = proof::at(row, "/source")?;
        let cell = proof::cell(source, "shipFrameTransform", state)?;
        assert_eq!(
            cell["sourceSetComplete"], true,
            "the original model names were all consulted"
        );
        let refs = proof::rows(cell, "/sourceUsesRefs")?;
        assert!(
            refs.iter()
                .filter_map(Value::as_str)
                .any(|value| value.contains("nodeMeshTypePrefix")),
            "an absent or ambiguous name retains the complete model node selection"
        );
        proof::cell(source, "localTransform", "present")?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn assert_original_model(catalog: &Value, cell: &Value) -> TestResult {
    let selected_path = format!("Data/{}", fixture::MODEL);
    let mut matrices = 0;
    for used in proof::uses(catalog, cell)? {
        if used.pointer("/location/field") != Some(&json!("localTransform")) {
            continue;
        }
        let original = proof::source(catalog, used)?;
        assert_eq!(
            original.pointer("/path/value"),
            Some(&json!(selected_path)),
            "the selected primary model supplied these exact matrix bytes"
        );
        assert_eq!(
            original["kind"], "archiveEntry",
            "the proof refers to an admitted physical archive entry"
        );
        matrices += 1;
    }
    assert_eq!(
        matrices, 2,
        "both matrices bind to the selected original primary model"
    );
    Ok(())
}

#[test]
fn catalog_preserves_both_physical_witnesses_of_an_ambiguous_model() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::AmbiguousModel)?;
    let catalog = build(&archive)?;
    let rows = interior_rows(&catalog)?;
    let row = rows.first().ok_or("original room facility")?;
    let cell = proof::cell(proof::at(row, "/source")?, "shipFrameTransform", "invalid")?;
    assert_eq!(cell["sourceSetComplete"], true);
    let mut found = Vec::new();
    let selected = format!("Data/{}", fixture::MODEL);
    for used in proof::uses(&catalog, cell)? {
        let source = proof::source(&catalog, used)?;
        let Some(path) = source.pointer("/path/value").and_then(Value::as_str) else {
            continue;
        };
        if path.eq_ignore_ascii_case(&selected) && !found.contains(&path) {
            found.push(path);
        }
    }
    assert_eq!(
        found.len(),
        2,
        "both matching directory Entries prove the ambiguity"
    );
    proof::closure(&catalog)
}
