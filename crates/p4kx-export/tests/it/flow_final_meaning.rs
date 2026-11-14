// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::flow_final_fixture::{self as fixture, Case};
use crate::flow_final_graph::Graph;
use crate::flow_final_original::{Configuration, SourceKind};
use crate::flow_final_scalar_source;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;

pub(crate) fn check(
    graph: &Graph<'_>,
    original: &Configuration,
    case: Case,
    index: usize,
) -> TestResult {
    selection(graph, original, case, index)?;
    for (index, source) in original.nodes.iter().enumerate() {
        let node = graph
            .values
            .get(index)
            .ok_or("actual retained source occurrence")?;
        let meaning = node
            .get("condition")
            .ok_or("required nullable interpretation")?;
        match &source.kind {
            SourceKind::Struct(name) if name == fixture::KNOWN => {
                assert_eq!(
                    meaning,
                    &json!({"type":"onMissionStart"}),
                    "known interpretation does not replace its extra original facts"
                );
                scalar_values(graph, node, matches!(case, Case::KnownNan))?;
            }
            SourceKind::Struct(name) if name == fixture::UNKNOWN => {
                assert_eq!(
                    meaning.get("type"),
                    Some(&json!("unsupported")),
                    "readable unknown keeps a separate unsupported interpretation"
                );
                scalar_values(graph, node, false)?;
            }
            SourceKind::Struct(name) if name == "MissionFlowCondition_AND" => {
                conjunction(graph, node, meaning)?;
            }
            _ => assert_eq!(
                meaning,
                &Value::Null,
                "non-condition source facts are not reinterpreted as conditions"
            ),
        }
    }
    Ok(())
}

fn scalar_values(graph: &Graph<'_>, node: &Value, nan: bool) -> TestResult {
    let fields = raw::array(raw::payload(node, "struct")?, "fields")?;
    assert_eq!(
        fields.len(),
        19,
        "all19 original declarations remain in the same arena"
    );
    for (field, (name, _, label)) in fields.iter().zip(flow_final_scalar_source::declarations()) {
        assert_eq!(
            field.pointer("/name/value"),
            Some(&json!(name)),
            "exact source field order/name"
        );
        assert_eq!(
            field.pointer("/declaration/dataType/value"),
            Some(&json!(label)),
            "exact storage labels, including uint and pointer types"
        );
    }
    integers(graph, node)?;
    floats(graph, node, nan)?;
    texts(graph, node)?;
    other(graph, node)
}

fn integers(graph: &Graph<'_>, node: &Value) -> TestResult {
    for (name, kind, expected) in [
        ("i8", "int8", json!(i8::MIN)),
        ("i16", "int16", json!(i16::MIN)),
        ("i32", "int32", json!(i32::MIN)),
        ("i64", "int64", json!(i64::MIN.to_string())),
        ("u8", "uint8", json!(u8::MAX)),
        ("u16", "uint16", json!(u16::MAX)),
        ("u32", "uint32", json!(u32::MAX)),
        ("u64", "uint64", json!(u64::MAX.to_string())),
    ] {
        let number = raw::payload(graph.field(node, name)?, "number")?
            .get("value")
            .ok_or("exact source number")?;
        assert_eq!(
            number.get("type"),
            Some(&json!(kind)),
            "original width {name}"
        );
        assert_eq!(
            number.get("value"),
            Some(&expected),
            "all original signed/unsigned digits {name}"
        );
    }
    Ok(())
}

fn floats(graph: &Graph<'_>, node: &Value, nan: bool) -> TestResult {
    for (name, kind, bits) in [
        ("single", "f32", if nan { "7fc01234" } else { "80000000" }),
        ("double", "f64", "8000000000000000"),
    ] {
        let number = raw::payload(graph.field(node, name)?, "number")?
            .get("value")
            .ok_or("original IEEE token")?;
        assert_eq!(
            number.get("type"),
            Some(&json!(kind)),
            "original float width"
        );
        assert_eq!(
            number.get("rawBits"),
            Some(&json!(bits)),
            "every original IEEE bit is retained"
        );
        if nan && name == "single" {
            assert_eq!(
                number.get("finite"),
                Some(&Value::Null),
                "nonfinite data retains exact bits without a fabricated finite projection"
            );
        } else {
            assert_eq!(
                number
                    .get("finite")
                    .and_then(Value::as_f64)
                    .ok_or("finite negative zero")?
                    .to_bits(),
                (-0.0_f64).to_bits(),
                "original zero sign remains exact"
            );
        }
    }
    Ok(())
}

fn texts(graph: &Graph<'_>, node: &Value) -> TestResult {
    for (name, kind, expected) in [
        ("text", "string", fixture::TEXT),
        ("locale", "locale", fixture::LOCALE),
    ] {
        let value = raw::payload(graph.field(node, name)?, kind)?
            .get("value")
            .ok_or("typed original text/key")?;
        assert_eq!(
            value.get("type"),
            Some(&json!("gameText")),
            "unclassified free text remains conservatively protected"
        );
        assert_eq!(
            value.get("sourceText"),
            Some(&json!(expected)),
            "raw spaces/macros/format text/key are inert and complete"
        );
    }
    assert_eq!(
        raw::payload(graph.field(node, "enum")?, "enum")?.get("value"),
        Some(&json!(fixture::ENUM)),
        "enum labels stay literal and never become localization lookup keys"
    );
    Ok(())
}

fn other(graph: &Graph<'_>, node: &Value) -> TestResult {
    assert_eq!(
        raw::payload(graph.field(node, "flag")?, "boolean")?.get("value"),
        Some(&json!(false)),
        "present false is not absence"
    );
    assert_eq!(
        raw::payload(graph.field(node, "guid")?, "guid")?.get("value"),
        Some(&json!("00000000-0000-0000-0000-000000000000")),
        "authored zero GUID remains present"
    );
    assert_eq!(
        raw::reference_guid(graph.field(node, "reference")?)?,
        fixture::TARGET,
        "stored GUID Reference remains distinct from physical navigation"
    );
    let weak = raw::payload(graph.field(node, "weak")?, "weakReference")?
        .get("value")
        .ok_or("actual weak reference")?;
    assert_eq!(
        weak.pointer("/target/type"),
        Some(&json!("instance")),
        "genuine instance applicability"
    );
    assert_eq!(
        weak.pointer("/target/sourceType/value"),
        Some(&json!("ExtraOwned")),
        "actual target instance type, not the root Record type"
    );
    assert_eq!(
        weak.pointer("/target/location/value"),
        Some(&json!("outsideOwned")),
        "actual original instance outside the Flow subtree"
    );
    for (name, expected) in [("class", -9), ("strong", -10)] {
        let marker = graph.field(graph.field(node, name)?, "marker")?;
        assert_eq!(
            raw::payload(marker, "number")?.pointer("/value/value"),
            Some(&json!(expected)),
            "additional nested facts remain exact"
        );
    }
    Ok(())
}

fn conjunction(graph: &Graph<'_>, node: &Value, meaning: &Value) -> TestResult {
    let array = graph.field(node, "conditions")?;
    let children = raw::elements(array)?;
    assert_eq!(
        children.len(),
        4,
        "all original slots participate in the explicit condition binding"
    );
    assert_eq!(
        meaning.get("type"),
        Some(&json!("and")),
        "actual recognized condition kind"
    );
    assert_eq!(
        meaning.get("conditionsField"),
        Some(&json!({"type":"field","fieldIndex":"0"})),
        "binding points to the actual declaration"
    );
    assert_eq!(
        meaning.get("children"),
        Some(&json!(children)),
        "same raw inventory retains null/error/equal occurrences without a recursive mirror"
    );
    assert_ne!(
        children.first(),
        children.last(),
        "equal source contents retain different local occurrence identities"
    );
    Ok(())
}
fn selection(graph: &Graph<'_>, original: &Configuration, case: Case, index: usize) -> TestResult {
    let expected = if original.declaration.is_none() {
        json!({"type":"absent"})
    } else if index != 0 {
        json!({"type":"shadowedDeclaration"})
    } else if matches!(case, Case::Null) {
        json!({"type":"noStruct"})
    } else {
        let root = original
            .nodes
            .first()
            .ok_or("actual selected original root")?;
        json!({"type":"selected","value":graph.owner.reference(root.ordinal)?})
    };
    assert_eq!(
        graph.configuration.get("selection"),
        Some(&expected),
        "first-declaration/first-struct interpretation is explicit, with later facts retained"
    );
    Ok(())
}
