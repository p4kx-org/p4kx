// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestMtl, TestMtlMaterial};
use serde_json::{Value, json};

use crate::exact_audio_assertions;
use crate::exact_material_assertions as check;
use crate::exact_material_fixture::{self as fixture, Encoding};
use crate::support::{Install, TestResult};

const COUNT: usize = 65;

fn name(ordinal: usize) -> String {
    format!("Param{ordinal:02}")
}

fn value(ordinal: usize, huge_tail: bool) -> String {
    if huge_tail && ordinal == COUNT - 1 {
        "V".repeat(300 << 10)
    } else {
        format!("  value{ordinal:02}\\n%% ?  ")
    }
}

fn install(encoding: Encoding, huge_tail: bool) -> TestResult<Install> {
    let material = (0..COUNT).fold(TestMtlMaterial::new("complete"), |material, ordinal| {
        material.public_param(&name(ordinal), &value(ordinal, huge_tail))
    });
    fixture::install(&TestMtl::single(&material), encoding, Vec::new())
}

fn original(install: &Install, huge_tail: bool) -> TestResult {
    let library = fixture::original(install)?;
    assert!(library.single, "one actual original material");
    assert_eq!(library.materials.len(), 1, "one actual parsed material");
    let material = library.materials.first().ok_or("original material")?;
    assert!(
        material.truncated.is_empty(),
        "the original reader did not cut any field"
    );
    assert_eq!(
        material.public_params.len(),
        COUNT,
        "all actual declared parameters"
    );
    for (ordinal, (actual_name, actual_value)) in material.public_params.iter().enumerate() {
        assert_eq!(
            *actual_name,
            name(ordinal),
            "actual original parameter order/name"
        );
        assert_eq!(
            *actual_value,
            value(ordinal, huge_tail),
            "exact original parameter token"
        );
    }
    Ok(())
}

#[test]
fn complete_material_parameters_keeps_the_actual65th_parameter() -> TestResult {
    for encoding in [Encoding::Text, Encoding::CryXml] {
        let install = install(encoding, false)?;
        original(&install, false)?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let result = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
        let material = check::material(&result)?;
        assert_eq!(
            material.get("public_param_count"),
            Some(&json!(COUNT)),
            "all original values count"
        );
        let parameters = material
            .get("public_params")
            .and_then(Value::as_array)
            .ok_or("complete parameters")?;
        assert_eq!(parameters.len(), COUNT, "no arbitrary parameter prefix");
        for (ordinal, parameter) in parameters.iter().enumerate() {
            assert_eq!(
                parameter.get("name"),
                Some(&json!(name(ordinal))),
                "original parameter name/order"
            );
            assert_eq!(
                parameter.get("value"),
                Some(&json!(value(ordinal, false))),
                "exact original token"
            );
        }
    }
    Ok(())
}

#[test]
fn complete_material_parameters_refuses_an_actual_huge65th_parameter() -> TestResult {
    let install = install(Encoding::Text, true)?;
    original(&install, true)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    exact_audio_assertions::too_large(
        &mut server,
        "mtl_summary",
        &json!({"path":fixture::MATERIAL}),
    )
}
