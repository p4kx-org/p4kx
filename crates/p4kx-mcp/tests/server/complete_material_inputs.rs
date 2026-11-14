// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::complete_material_inputs_source::{self as source, Kind};
use crate::exact_material_assertions as check;
use crate::exact_material_fixture;
use crate::support::TestResult;

fn complete(kind: Kind) -> TestResult {
    let install = source::install(kind)?;
    source::original(&install, kind)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok(
        "mtl_summary",
        &json!({"path":exact_material_fixture::MATERIAL}),
    )?;
    let material = check::material(&result)?;
    assert_eq!(
        material.get(kind.count_field()),
        Some(&json!(source::COUNT)),
        "full count agrees with actual original XML declarations"
    );
    let items = material
        .get(kind.field())
        .and_then(Value::as_array)
        .ok_or("all original items")?;
    assert_eq!(items.len(), source::COUNT, "no source or MCP prefix loss");
    for (ordinal, item) in items.iter().enumerate() {
        for (_, output, value) in kind.attributes(ordinal) {
            assert_eq!(
                item.get(output),
                Some(&json!(value)),
                "exact original datum/order"
            );
        }
        if matches!(kind, Kind::Layers) {
            assert_eq!(
                item.get("palette_tint"),
                Some(&json!("1")),
                "exact original layer tint"
            );
        }
    }
    Ok(())
}

#[test]
fn complete_material_keeps_all65_original_texture_slots() -> TestResult {
    complete(Kind::Textures)
}

#[test]
fn complete_material_keeps_all65_original_layers() -> TestResult {
    complete(Kind::Layers)
}
