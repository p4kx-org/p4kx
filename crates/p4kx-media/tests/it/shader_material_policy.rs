// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Exclusion, Filter};
use p4kx_media::MediaKind;
use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::shader_material_support::{self as support, TestResult};

const FLAGS: &[u8] = b"Property { Name = %A\n }\n";

#[test]
fn target_filter_and_kind_selection_are_not_automatic_inclusion() -> TestResult {
    let entries = || {
        vec![
            support::material("demo", "%A"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ]
    };
    let mut options = support::options();
    options.include = Some(Filter::parse("*.mtl")?);
    let result = support::run(entries(), &options)?;
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "not-selected");
        assert_eq!(link["reference"]["resolution"]["reason"], "filter");
        assert!(link["input"].is_null());
    }
    options.include = None;
    options.kinds = vec![MediaKind::Material];
    let result = support::run(entries(), &options)?;
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "not-selected");
        assert_eq!(
            link["reference"]["resolution"]["reason"],
            "kind-not-requested"
        );
    }
    assert!(result.shaders.is_empty());
    Ok(())
}

#[test]
fn an_excluded_container_keeps_unknown_inventory_distinct_from_missing() -> TestResult {
    let mut options = support::options();
    options.exclude = Exclusion::parse(["*.pak"])?;
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            TestEntry::stored("Data/hidden.pak", b"not a container".to_vec()),
        ],
        &options,
    )?;
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "unavailable");
        assert_eq!(
            link["reference"]["resolution"]["reason"]["type"],
            "incomplete-inventory"
        );
        assert_eq!(link["reference"]["candidates_complete"], false);
    }
    Ok(())
}

#[test]
fn excluded_definition_bytes_are_never_parsed_for_a_property_target() -> TestResult {
    let mut options = support::options();
    options.exclude = Exclusion::parse(["*.ext"])?;
    let result = support::run(
        vec![
            support::material("demo", "%A"),
            support::container("Data/shaders.pak", "demo", &[0xff, 0xfe]),
        ],
        &options,
    )?;
    for link in support::links(&result)? {
        assert_eq!(link["reference"]["resolution"]["status"], "not-selected");
        assert!(link["input"].is_null());
    }
    assert!(result.index.complete);
    Ok(())
}

#[test]
fn unsupported_host_paths_and_flag_expressions_keep_the_original_tokens() -> TestResult {
    let result = support::run(
        vec![
            support::material("C:\\outside\\demo", "%A + B"),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    let links = support::links(&result)?;
    assert_eq!(
        links[0]["reference"]["original"]["value"],
        "C:\\outside\\demo"
    );
    assert_eq!(links[1]["reference"]["original"]["value"], "%A + B");
    for link in links {
        assert_eq!(link["reference"]["resolution"]["status"], "unavailable");
        assert_eq!(
            link["reference"]["resolution"]["reason"]["type"],
            "invalid-identifier"
        );
        assert!(link["input"].is_null());
    }
    Ok(())
}

#[test]
fn unsupported_property_names_and_unproved_common_flags_do_not_become_missing() -> TestResult {
    for definition in [
        b"Property { Name = %A | %B\n }\n".as_slice(),
        b"UsesCommonGlobalFlags\nProperty { Name = %A\n }\n".as_slice(),
    ] {
        let result = support::run(
            vec![
                support::material("demo", "%ABSENT"),
                support::container("Data/shaders.pak", "demo", definition),
            ],
            &support::options(),
        )?;
        let links = support::links(&result)?;
        assert_eq!(links[1]["reference"]["resolution"]["status"], "unavailable");
        assert_eq!(
            links[1]["reference"]["resolution"]["reason"]["type"],
            "unsupported-layout"
        );
    }
    Ok(())
}

#[test]
fn empty_absent_unknown_and_repeated_mtl_attributes_are_not_fabricated_defaults() -> TestResult {
    let bytes = br#"<Material><SubMaterials><Unknown/><Material/><Material Shader=""/><Material Shader="demo" SHADER="missing" StringGenMask="%A"/></SubMaterials></Material>"#;
    let result = support::run(
        vec![
            TestEntry::stored("Data/materials.mtl", bytes.to_vec()),
            support::container("Data/shaders.pak", "demo", FLAGS),
        ],
        &support::options(),
    )?;
    let links = support::links(&result)?;
    assert!(
        links
            .iter()
            .all(|link| link["material"] != 0 && link["material"] != 1)
    );
    assert_eq!(
        links.iter().filter(|link| link["material"] == 2).count(),
        1,
        "explicit empty Shader remains authored"
    );
    let repeated: Vec<_> = links.iter().filter(|link| link["material"] == 3).collect();
    assert_eq!(repeated.len(), 3);
    assert_eq!(repeated[0]["reference"]["original"]["value"], "demo");
    assert_eq!(repeated[1]["reference"]["original"]["value"], "missing");
    assert!(repeated[2]["shader"].is_null());
    assert_eq!(
        repeated[2]["reference"]["resolution"]["status"],
        "unavailable"
    );
    Ok(())
}

#[test]
fn common_flag_scope_keeps_proved_local_links_and_unknown_absent_flags() -> TestResult {
    let definition = b"UsesCommonGlobalFlags\nProperty { Name = %A\n }\n";
    let result = support::run(
        vec![
            support::material("demo", "%A%ABSENT"),
            support::container("Data/flags.pak", "demo", definition),
        ],
        &support::options(),
    )?;
    assert!(result.index.complete);
    let mut properties = result
        .shaders
        .iter()
        .filter(|row| row.subresource.namespace == "shader-property");
    let property = properties.next().ok_or("actual emitted local Property")?;
    assert!(
        properties.next().is_none(),
        "the fixture emits exactly one local Property"
    );
    let identity = serde_json::to_value(&property.source.identity)?;
    let subresource = serde_json::to_value(&property.subresource)?;
    let digest = serde_json::to_value(
        &property
            .source
            .raw
            .as_ref()
            .ok_or("retained local Property source")?
            .digest,
    )?;
    let links = support::links(&result)?;
    let local = links
        .iter()
        .find(|link| link["role"] == "flag" && link["reference"]["original"]["value"] == "%A")
        .ok_or("original selected local flag")?;
    let missing = links
        .iter()
        .find(|link| link["role"] == "flag" && link["reference"]["original"]["value"] == "%ABSENT")
        .ok_or("original selected absent flag")?;
    assert_eq!(local["reference"]["resolution"]["status"], "resolved");
    assert_eq!(
        local["reference"]["resolution"]["target"]["source"],
        identity
    );
    assert_eq!(
        local["reference"]["resolution"]["target"]["subresource"], subresource,
        "the common marker does not replace the emitted local declaration identity"
    );
    assert_eq!(missing["reference"]["resolution"]["status"], "unavailable");
    assert_eq!(
        missing["reference"]["resolution"]["reason"]["type"],
        "unsupported-layout"
    );
    for (link, token, position) in [(local, "%A", 0), (missing, "%ABSENT", 1)] {
        assert_flag_receipt(link, token, position, &identity, &digest)?;
    }
    Ok(())
}

fn assert_flag_receipt(
    link: &Value,
    token: &str,
    position: u64,
    identity: &Value,
    digest: &Value,
) -> TestResult {
    assert_eq!(
        link.pointer("/reference/original/value")
            .and_then(Value::as_str),
        Some(token),
        "the original flag token survives"
    );
    assert_eq!(
        link.get("shader").and_then(Value::as_str),
        Some("demo"),
        "the original shader scope survives"
    );
    assert_eq!(
        link.get("token_index").and_then(Value::as_u64),
        Some(position),
        "the authored token position survives"
    );
    assert_eq!(
        link.pointer("/input/omission").and_then(Value::as_str),
        Some("reference-lookup"),
        "a private read has an explicit omission role"
    );
    assert_eq!(
        link.pointer("/input/source/identity"),
        Some(identity),
        "the receipt uses the actual local source"
    );
    assert_eq!(
        link.pointer("/input/digest"),
        Some(digest),
        "the receipt measures the complete actual source"
    );
    assert_eq!(
        link.pointer("/input/use_at/namespace")
            .and_then(Value::as_str),
        Some("material-shader-reference"),
        "the read belongs to the material declaration"
    );
    let indices = link
        .pointer("/input/use_at/indices")
        .and_then(Value::as_array)
        .ok_or("actual use location")?;
    assert_eq!(
        indices.last().ok_or("selected-token position")?.as_u64(),
        Some(position),
        "the read belongs to the exact selected token"
    );
    Ok(())
}
