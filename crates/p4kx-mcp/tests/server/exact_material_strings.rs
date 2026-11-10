// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestMtl, TestMtlMaterial};
use serde_json::{Value, json};

use crate::exact_material_assertions as check;
use crate::exact_material_fixture::{self as fixture, Encoding, TexturePath};
use crate::support::TestResult;

#[test]
fn exact_material_strings_and_texture_paths_roundtrip_for_both_xml_encodings() -> TestResult {
    for encoding in [Encoding::Text, Encoding::CryXml] {
        let texts = fixture::texts(TexturePath::Reusable);
        let install = fixture::exact(&texts, encoding)?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let summary = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
        check::exact(&summary, &texts)?;
        check::texture_roundtrip(&mut server, &summary)?;
    }
    Ok(())
}

#[test]
fn exact_material_long_returned_path_has_an_explicit_input_transport_limit() -> TestResult {
    let texts = fixture::texts(TexturePath::OverInputLimit);
    let install = fixture::exact(&texts, Encoding::Text)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let summary = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
    check::exact(&summary, &texts)?;
    let relative = check::material(&summary)?
        .pointer("/textures/0/file")
        .and_then(Value::as_str)
        .ok_or("exact returned long path")?;
    let error = server.error(
        "read_entry_bytes",
        &json!({"path":fixture::entry_path(relative)}),
    )?;
    assert!(
        error.contains("path") && error.contains("is longer than 1024 bytes"),
        "the real input constraint is explicit, never a shortened lookup: {error}"
    );
    Ok(())
}

#[test]
fn exact_material_oversize_scalar_or_total_text_is_explicitly_refused() -> TestResult {
    let huge = "x".repeat(300 * 1024);
    let sum = "y".repeat(100 * 1024);
    for library in [
        fixture::parameter_library(&[("single", &huge)]),
        TestMtl::single(
            &TestMtlMaterial::new(&sum)
                .shader(&sum)
                .public_param("Value", &sum),
        ),
    ] {
        let install = fixture::install(&library, Encoding::Text, Vec::new())?;
        let original = fixture::original(&install)?;
        let material = original
            .materials
            .first()
            .ok_or("actual original oversize material")?;
        let value = &material
            .public_params
            .first()
            .ok_or("original oversize parameter")?
            .1;
        if material.name == "single" {
            assert_eq!(
                value, &huge,
                "the original single value is fully present before response refusal"
            );
        } else {
            assert_eq!(
                (&material.name, &material.shader, value),
                (&sum, &sum, &sum),
                "all three original text occurrences genuinely exist"
            );
        }
        let bytes = material.name.len()
            + material.shader.len()
            + material
                .public_params
                .iter()
                .map(|(_, value)| value.len())
                .sum::<usize>();
        assert!(
            bytes >= 300 * 1024,
            "the original data genuinely exceeds the documented response bound"
        );
        let mut server = install.serve(&["--archive", "LIVE"])?;
        check::too_large(&mut server, &json!({"path":fixture::MATERIAL}))?;
    }
    Ok(())
}

#[test]
fn exact_material_paging_preserves_full_values_and_original_order() -> TestResult {
    let first = "a".repeat(140 * 1024);
    let second = "b".repeat(140 * 1024);
    let install = fixture::install(
        &fixture::parameter_library(&[("first", &first), ("second", &second)]),
        Encoding::Text,
        Vec::new(),
    )?;
    let original = fixture::original(&install)?;
    assert_eq!(
        original.materials.len(),
        2,
        "two actual original submaterials"
    );
    for (material, expected) in original.materials.iter().zip([&first, &second]) {
        assert_eq!(
            material.public_params.first().map(|(_, value)| value),
            Some(expected),
            "complete original ordered parameter values"
        );
    }
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let one = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
    assert_eq!(
        one.get("materials").and_then(Value::as_array).map(Vec::len),
        Some(1),
        "only complete items fitting the existing response budget are returned"
    );
    assert_eq!(
        check::material(&one)?.pointer("/public_params/0/value"),
        Some(&json!(first)),
        "the first value is never shortened to force more items"
    );
    let cursor = one
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("continuation after the complete first item")?;
    let two = server.ok(
        "mtl_summary",
        &json!({"path":fixture::MATERIAL,"cursor":cursor}),
    )?;
    assert_eq!(
        check::material(&two)?.get("id"),
        Some(&json!(1)),
        "actual original second ordinal"
    );
    assert_eq!(
        check::material(&two)?.pointer("/public_params/0/value"),
        Some(&json!(second)),
        "the continuation retains the whole original second value"
    );
    assert!(
        two.get("next_cursor").is_none(),
        "there are no omitted later source items"
    );
    Ok(())
}

#[test]
fn exact_material_unshown_siblings_do_not_spend_the_item_text_budget() -> TestResult {
    let huge = "z".repeat(300 * 1024);
    let mut material = TestMtlMaterial::new("shown");
    for index in 0..64 {
        material = material.public_param(&format!("Field{index}"), "kept");
    }
    material = material.public_param("Unshown", &huge);
    let install = fixture::install(&TestMtl::single(&material), Encoding::Text, Vec::new())?;
    let original = fixture::original(&install)?;
    assert_eq!(
        original
            .materials
            .first()
            .ok_or("original first material")?
            .public_params
            .len(),
        65,
        "the original unshown sibling actually exists"
    );
    let parameters = &original
        .materials
        .first()
        .ok_or("original material")?
        .public_params;
    let (name, value) = parameters
        .get(64)
        .ok_or("actual original sixty-fifth parameter")?;
    assert_eq!(
        name, "Unshown",
        "the unmaterialized source sibling has its actual original name"
    );
    assert_eq!(
        value, &huge,
        "its complete original huge value exists before the MCP call"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let summary = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
    let shown = check::material(&summary)?;
    assert_eq!(
        shown.get("public_param_count"),
        Some(&json!(65)),
        "the independent original total is retained"
    );
    assert_eq!(
        shown
            .get("public_params")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(64),
        "this batch preserves the existing explicit list preview policy"
    );
    unshown_material(&huge)
}

fn unshown_material(huge: &str) -> TestResult {
    let install = fixture::install(
        &fixture::parameter_library(&[("shown", "small"), ("later", huge)]),
        Encoding::Text,
        Vec::new(),
    )?;
    let original = fixture::original(&install)?;
    assert_eq!(
        original
            .materials
            .get(1)
            .and_then(|material| material.public_params.first())
            .map(|(_, value)| value.as_str()),
        Some(huge),
        "the actual later item really is large"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let summary = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL,"limit":1}))?;
    assert_eq!(
        check::material(&summary)?.get("name"),
        Some(&json!("shown")),
        "a later unmaterialized sibling cannot refuse the current item"
    );
    let cursor = summary
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("actual later item cursor")?;
    assert!(
        !cursor.is_empty(),
        "the actual later item retains a continuation without being materialized"
    );
    Ok(())
}
