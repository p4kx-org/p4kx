// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::exact_material_fixture::{self as fixture, Texts};
use crate::support::{Server, TestResult};

pub(crate) fn material(summary: &Value) -> TestResult<&Value> {
    summary
        .get("materials")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| "actual summary material".into())
}

pub(crate) fn exact(summary: &Value, texts: &Texts) -> TestResult {
    assert_eq!(
        summary.get("default_palette"),
        Some(&json!(texts.palette)),
        "the original palette path is exact"
    );
    let material = material(summary)?;
    for (field, expected) in [
        ("name", &texts.name),
        ("shader", &texts.shader),
        ("surface_type", &texts.surface),
        ("string_gen_mask", &texts.mask),
    ] {
        assert_eq!(
            material.get(field),
            Some(&json!(expected)),
            "the entire original {field} is returned"
        );
    }
    for (path, expected) in [
        ("/textures/0/map", &texts.map),
        ("/textures/0/file", &texts.file),
        ("/public_params/0/name", &texts.parameter),
        ("/public_params/0/value", &texts.value),
        ("/layers/0/name", &texts.layer),
        ("/layers/0/path", &texts.layer_path),
        ("/layers/0/palette_tint", &texts.tint),
    ] {
        assert_eq!(
            material.pointer(path),
            Some(&json!(expected)),
            "complete original datum at {path}"
        );
    }
    Ok(())
}

pub(crate) fn texture_roundtrip(server: &mut Server, summary: &Value) -> TestResult {
    let relative = material(summary)?
        .pointer("/textures/0/file")
        .and_then(Value::as_str)
        .ok_or("returned texture path")?;
    let entry = fixture::entry_path(relative);
    let response = server.ok("read_entry_bytes", &json!({"path":entry}))?;
    let bytes = STANDARD.decode(
        response
            .get("data")
            .and_then(Value::as_str)
            .ok_or("original texture bytes")?,
    )?;
    assert_eq!(
        bytes,
        fixture::PAYLOAD,
        "the returned full relative texture path roundtrips through the documented Data prefix"
    );
    Ok(())
}

pub(crate) fn too_large(server: &mut Server, arguments: &Value) -> TestResult {
    let error = server.error("mtl_summary", arguments)?;
    assert!(
        error.contains("response cap of 262144 bytes"),
        "the existing response budget is explicit: {error}"
    );
    assert!(
        error.len() < 1024,
        "failure metadata must not copy the huge source value/name"
    );
    Ok(())
}
