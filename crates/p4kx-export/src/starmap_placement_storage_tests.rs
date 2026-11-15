// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ptr;
use std::sync::Arc;

use p4kx_testkit::TestElement;
use serde_json::json;

use super::{Limits, external_at, read, root, system};

#[test]
fn repeated_container_occurrences_share_only_source_owned_data() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let system_xml = root(vec![
        external_at("tile", "10,0,0"),
        external_at("tile", "20,0,0"),
    ]);
    let tile = root(vec![
        TestElement::new("Child")
            .attribute("pos", "1,0,0")
            .attribute("locationType", "2")
            .attribute("label", "Storage canary"),
    ]);
    let mut placements = read(
        dir.path(),
        system(&system_xml, &[("tile", tile)]),
        Limits::default(),
    );
    let containers = placements.take_instances();
    let [_, first, second] = containers.rows.as_slice() else {
        return Err("expected the system and two physical occurrences of one tile".into());
    };
    let first_wire = serde_json::to_value(first)?;
    let texts = first
        .text_cells_for_test()
        .into_iter()
        .zip(second.text_cells_for_test());
    for (field, (left, right)) in ["system", "sourcePath", "declaredPath"]
        .into_iter()
        .zip(texts)
    {
        let left_text = left.value().ok_or("present occurrence text")?;
        let right_text = right.value().ok_or("present repeated occurrence text")?;
        assert!(
            ptr::eq(left_text.as_str(), right_text.as_str()),
            "shared bytes for {field}"
        );
        let original_cell = left.map_ref(|text| text.as_str().to_owned());
        assert_eq!(
            first_wire.get(field),
            Some(&serde_json::to_value(original_cell)?),
            "original wire text, state and origin for {field}"
        );
    }
    let [system_name, _, first_path] = first.text_cells_for_test();
    let [repeated_system_name, _, repeated_path] = second.text_cells_for_test();
    assert!(
        ptr::eq(
            system_name.origin().source_uses_refs(),
            repeated_system_name.origin().source_uses_refs()
        ),
        "one sourced system cell is reused"
    );
    assert_ne!(
        first_path.origin(),
        repeated_path.origin(),
        "identical reference text retains its distinct authored slots"
    );
    verify_occurring_locations(&placements.take_locations().rows)
}

fn verify_occurring_locations(
    rows: &[crate::starmap_containers::location_instance::LocationInstance],
) -> Result<(), Box<dyn Error>> {
    let [first, second] = rows else {
        return Err("expected one typed location per physical tile occurrence".into());
    };
    let declaration = first.declaration_for_test();
    assert!(
        Arc::ptr_eq(declaration, second.declaration_for_test()),
        "one immutable declaration is shared across two routes"
    );
    let original = serde_json::to_value(declaration.as_ref())?;
    let first = serde_json::to_value(first)?;
    let second = serde_json::to_value(second)?;
    for row in [&first, &second] {
        assert_eq!(
            row.get("declaration"),
            Some(&original),
            "original wire declaration"
        );
    }
    assert_ne!(
        first.pointer("/instanceId/value"),
        second.pointer("/instanceId/value"),
        "both routes retain physical identities"
    );
    assert_eq!(
        first.pointer("/worldPosition/value"),
        Some(&json!([11.0, 0.0, 0.0])),
        "first composed placement"
    );
    assert_eq!(
        second.pointer("/worldPosition/value"),
        Some(&json!([21.0, 0.0, 0.0])),
        "second composed placement"
    );
    Ok(())
}
