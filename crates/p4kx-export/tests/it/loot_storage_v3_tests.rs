// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use jsonschema::Validator;
use p4kx_export::{CatalogOptions, ExportHealthStatus};
use p4kx_extraction::{SourceInventory, SourceUseRole};
use serde_json::{Value, json};

use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_storage_v3_fixture as fixture;
use crate::loot_storage_v3_source::{self as source, Original};
use crate::loot_v3_graph::{self as graph, Graph};
use crate::loot_v3_origin::{self as origin, ExpectedOrigin};

#[test]
fn all19_original_storage_types_and_text_domains_survive_each_real_input_origin() -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        check_input(input, &validator)?;
    }
    Ok(())
}

fn check_input(input: Input, validator: &Validator) -> TestResult {
    let fixture = fixture::open(input)?;
    let original = source::check(&fixture, input)?;
    assert_eq!(
        fixture.entry.origin().matches_archive(&fixture.archive),
        !matches!(input, Input::Standalone),
        "actual physical source lineage"
    );
    let built = p4kx_export::build_catalog_from(
        &fixture.archive,
        &fixture.entry,
        &CatalogOptions::new(UNIX_EPOCH),
    )?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "all actual source variants remain complete"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert!(
        validator.is_valid(&catalog),
        "the full actual producer document satisfies canonical v3 schema"
    );
    let owner = Graph::new(&catalog, fixture::OWNER)?;
    declarations(&catalog, &original)?;
    integers(&owner)?;
    floats(&owner)?;
    texts(&owner)?;
    other(&owner)?;
    origins(&owner, &original, built.summary().source_inventory(), input)
}

fn origins(
    owner: &Graph<'_>,
    original: &Original,
    inventory: &SourceInventory,
    input: Input,
) -> TestResult {
    for (name, _, _) in source::declarations() {
        let node = owner.root(name)?;
        let uses = [origin::value_use(
            original.ordinal(name)?,
            "$value",
            SourceUseRole::Payload,
        )];
        origin::verify(
            node.get("value").ok_or("one immediate sourced datum")?,
            inventory,
            &ExpectedOrigin {
                encrypted: input.marker(),
                complete: true,
                uses: &uses,
            },
        )
        .map_err(|problem| format!("original {name} immediate origin: {problem:?}"))?;
    }
    Ok(())
}

fn declarations(catalog: &Value, original: &Original) -> TestResult {
    let rows = catalog
        .pointer("/definitions/lootPoolFilters/rows")
        .and_then(Value::as_array)
        .ok_or("complete canonical physical rows")?;
    let row = rows
        .iter()
        .find(|row| row.pointer("/guid/value") == Some(&json!(fixture::OWNER)))
        .ok_or("actual source owner row")?;
    assert_eq!(
        row.get("values").and_then(Value::as_array).map(Vec::len),
        Some(26),
        "all original occurrences survive"
    );
    let fields = row
        .get("fields")
        .and_then(Value::as_array)
        .ok_or("original ordered declarations")?;
    assert_eq!(fields.len(), 22, "extra original fields are not dropped");
    for (field, (name, _, label)) in fields.iter().zip(source::declarations()) {
        assert_eq!(
            graph::present(field.get("name").ok_or("source name")?)?,
            &json!(name),
            "original declaration order/name"
        );
        assert_eq!(
            graph::present(
                field
                    .pointer("/declaration/dataType")
                    .ok_or("source storage")?
            )?,
            &json!(label),
            "exact original type label"
        );
        assert_eq!(
            graph::present(
                field
                    .pointer("/declaration/isArray")
                    .ok_or("source array flag")?
            )?,
            &json!(false),
            "exact original scalar declaration"
        );
        assert_eq!(
            graph::present(
                field
                    .pointer("/declaration/target")
                    .ok_or("retained declaration target")?
            )?,
            &json!(original.target(name)?),
            "exact original decoder-minted target word"
        );
    }
    Ok(())
}

fn integers(owner: &Graph<'_>) -> TestResult {
    for (name, kind, value) in [
        ("i8", "int8", json!(i8::MIN)),
        ("i16", "int16", json!(i16::MIN)),
        ("i32", "int32", json!(i32::MIN)),
        ("i64", "int64", json!(i64::MIN.to_string())),
        ("signedMax", "int64", json!(i64::MAX.to_string())),
        ("u8", "uint8", json!(u8::MAX)),
        ("u16", "uint16", json!(u16::MAX)),
        ("u32", "uint32", json!(u32::MAX)),
        ("u64", "uint64", json!(u64::MAX.to_string())),
    ] {
        let number = graph::payload(owner.root(name)?, "number")?
            .get("value")
            .ok_or("typed original number")?;
        assert_eq!(
            number.get("type"),
            Some(&json!(kind)),
            "exact original integer width {name}"
        );
        assert_eq!(
            number.get("value"),
            Some(&value),
            "exact original signed/unsigned digits {name}"
        );
    }
    Ok(())
}

fn floats(owner: &Graph<'_>) -> TestResult {
    for (name, kind, bits) in [
        ("single", "f32", "80000000"),
        ("double", "f64", "8000000000000000"),
    ] {
        let number = graph::payload(owner.root(name)?, "number")?
            .get("value")
            .ok_or("typed original float")?;
        assert_eq!(
            number.get("type"),
            Some(&json!(kind)),
            "actual float storage width"
        );
        assert_eq!(
            number.get("rawBits"),
            Some(&json!(bits)),
            "all original negative-zero IEEE bits"
        );
        assert_eq!(
            number
                .get("finite")
                .and_then(Value::as_f64)
                .ok_or("finite decimal zero")?
                .to_bits(),
            (-0.0_f64).to_bits(),
            "finite projection retains the original negative sign"
        );
    }
    Ok(())
}

fn texts(owner: &Graph<'_>) -> TestResult {
    for (name, kind, expected) in [
        ("text", "string", fixture::TEXT),
        ("locale", "locale", fixture::LOCALE),
        ("unitFormat", "string", fixture::TEMPLATE),
        ("empty", "string", ""),
    ] {
        text(owner.root(name)?, kind, "gameText", expected)?;
    }
    known_texts(owner)
}

fn known_texts(owner: &Graph<'_>) -> TestResult {
    let class = graph::structure(owner.root("class")?, "CraftingGameplayPropertyDef")?;
    text(
        owner.field(class, "propertyName")?,
        "string",
        "public",
        "temperature",
    )?;
    text(
        owner.field(class, "unitFormat")?,
        "locale",
        "public",
        "%.2f RPM",
    )?;
    let strong = graph::structure(owner.root("strong")?, "PoolFilterInstance")?;
    text(
        owner.field(strong, "name")?,
        "string",
        "public",
        "@designer.literal",
    )?;
    text(
        owner.field(strong, "mode")?,
        "string",
        "public",
        "Cumulative",
    )
}

fn text(node: &Value, kind: &str, domain: &str, expected: &str) -> TestResult {
    let value = graph::payload(node, kind)?
        .get("value")
        .ok_or("exact original text encoding")?;
    assert_eq!(
        value.get("type"),
        Some(&json!(domain)),
        "actual owner-qualified text domain, not a field-name heuristic"
    );
    assert_eq!(
        value.get("sourceText"),
        Some(&json!(expected)),
        "no clipping, translation or normalization of original text/key"
    );
    Ok(())
}

fn other(owner: &Graph<'_>) -> TestResult {
    assert_eq!(
        graph::payload(owner.root("flag")?, "boolean")?.get("value"),
        Some(&json!(false)),
        "false is a present source value"
    );
    assert_eq!(
        graph::payload(owner.root("guid")?, "guid")?.get("value"),
        Some(&json!("00000000-0000-0000-0000-000000000000")),
        "zero GUID is present, not null"
    );
    assert_eq!(
        graph::payload(owner.root("enum")?, "enum")?.get("value"),
        Some(&json!(fixture::ENUM)),
        "enum label stays literal without a text-domain rewrite"
    );
    assert_eq!(
        graph::reference_guid(owner.root("reference")?)?,
        fixture::TARGET,
        "real stored Reference remains separate"
    );
    let weak = graph::payload(owner.root("weak")?, "weakReference")?
        .get("value")
        .ok_or("actual weak target")?;
    assert_eq!(
        weak.pointer("/target/type"),
        Some(&json!("instance")),
        "actual instance applicability"
    );
    assert_eq!(
        graph::present(
            weak.pointer("/target/sourceType")
                .ok_or("actual instance type")?
        )?,
        &json!("PoolFilterInstance"),
        "actual weak instance type"
    );
    Ok(())
}
