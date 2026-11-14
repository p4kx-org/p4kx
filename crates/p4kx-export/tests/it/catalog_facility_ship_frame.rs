// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{TestResult, build, fixture, interior_rows, proof};

#[test]
fn catalog_composes_original_room_and_child_coordinates_into_the_ship_frame() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::ShipFrame)?;
    let catalog = build(&archive)?;
    let rows = interior_rows(&catalog)?;
    let [root, invalid, first_child, _, second_child, _] = rows.as_slice() else {
        return Err("root and two original children, each with two SOC entity slots".into());
    };
    for (row, expected) in [
        (root, [8.0, 21.0, 33.0]),
        (first_child, [9.0, 21.0, 37.0]),
        (second_child, [9.0, 29.0, 37.0]),
    ] {
        assert_ship_translation(proof::at(row, "/source")?, expected)?;
    }
    proof::cell(
        proof::at(invalid, "/source")?,
        "shipFrameTransform",
        "invalid",
    )?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)
}

#[test]
fn catalog_keeps_ship_frame_states_when_the_original_inputs_cannot_supply_a_matrix() -> TestResult {
    for (case, state) in [
        (fixture::Case::MissingOffset, "absent"),
        (fixture::Case::InvalidOffset, "invalid"),
        (fixture::Case::Direct, "unsupported"),
        (fixture::Case::ItemRooms, "unsupported"),
    ] {
        let (_folder, archive) = fixture::archive(case)?;
        let catalog = build(&archive)?;
        let rows = interior_rows(&catalog)?;
        let row = rows.first().ok_or("original room entity")?;
        let cell = proof::cell(proof::at(row, "/source")?, "shipFrameTransform", state)?;
        assert!(
            cell["value"].is_null(),
            "{state} must not fabricate a ship-frame transform"
        );
        assert_eq!(
            cell["sourceSetComplete"],
            state != "unsupported",
            "missing frame dependencies are distinct from completely read invalid input"
        );
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn assert_ship_translation(source: &Value, expected: [f64; 3]) -> TestResult {
    let cell = proof::cell(source, "shipFrameTransform", "present")?;
    assert_eq!(
        cell["containsEncryptedSource"], true,
        "room stream is encrypted"
    );
    assert_eq!(
        cell["sourceSetComplete"], true,
        "all original matrix inputs were consumed"
    );
    let rows = proof::rows(cell, "/value")?;
    for (row, expected) in rows.iter().zip(expected) {
        let actual = proof::at(row, "/3")?
            .as_f64()
            .ok_or("matrix translation component")?;
        assert!(
            (actual - expected).abs() < 1e-5,
            "expected original room / Child / entity product {expected}, got {actual}"
        );
    }
    let refs = proof::rows(cell, "/sourceUsesRefs")?;
    assert!(
        refs.iter()
            .filter(|value| value.as_str().is_some_and(|s| {
                s.contains(":payload:s14:dataforgeValue") && s.ends_with(":$value")
            }))
            .count()
            >= 6,
        "ship frame retains the six physical authored offset payload contributions"
    );
    assert_eq!(
        source.pointer("/localTransform/value/0/3"),
        Some(&json!(1.0)),
        "existing entity-local transform is independent of the ship frame"
    );
    Ok(())
}
