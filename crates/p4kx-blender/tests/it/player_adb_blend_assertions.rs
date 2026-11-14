// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::BodyVariant;
use p4kx_blender::PlayerSourceCapture;
use p4kx_extraction::CancelToken;
use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::super::fixture_with;
use super::{
    BRANCH, Documents, TestResult, assert_attribute_evidence, assert_distinct_attribute_positions,
    assert_selector,
};

pub(super) fn assert_blends(
    layer: &Value,
    standalone: &Value,
    capture: &Value,
    source: &Value,
) -> TestResult {
    let blends = layer
        .get("blends")
        .and_then(Value::as_array)
        .ok_or("positioned Blend nodes")?;
    let [first, second] = blends.as_slice() else {
        return Err("expected both original Blend elements".into());
    };
    assert_eq!(
        first.get("ordinal"),
        Some(&json!(0)),
        "first original child"
    );
    assert_eq!(
        second.get("ordinal"),
        Some(&json!(5)),
        "second Blend keeps authored order"
    );
    assert_eq!(
        first.pointer("/position/value/node"),
        Some(&json!(8)),
        "exact CryXML Blend node"
    );
    assert_selector(first.get("duration").ok_or("duration")?, "present", &["1"])?;
    assert_selector(
        first.get("startTime").ok_or("start time")?,
        "present",
        &[""],
    )?;
    assert_selector(
        first.get("exitTime").ok_or("exit time")?,
        "present",
        &[".5"],
    )?;
    assert_selector(second.get("startTime").ok_or("start time")?, "absent", &[])?;
    assert_attribute_evidence(
        first.get("duration").ok_or("duration")?,
        capture,
        source,
        [8, 2],
        "Duration",
    )?;
    assert_flags(layer, standalone)?;
    assert_blend_relations(layer, standalone)
}

fn assert_blend_relations(layer: &Value, standalone: &Value) -> TestResult {
    let animations = layer
        .get("animations")
        .and_then(Value::as_array)
        .ok_or("original animations")?;
    for (animation, earlier) in animations.iter().zip([0, 0, 0, 0, 5]) {
        let relation = animation
            .get("precedingBlend")
            .ok_or("same-layer relationship")?;
        assert_eq!(
            relation.get("value"),
            Some(&json!(earlier)),
            "nearest earlier original Blend, including across an intervening sibling"
        );
        assert_eq!(
            relation.get("containsEncryptedSource"),
            Some(&json!(true)),
            "relation uses the encrypted XML"
        );
        assert_eq!(
            relation.get("sourceSetComplete"),
            Some(&json!(true)),
            "all three original element uses are admitted"
        );
        assert_eq!(
            relation
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(3),
            "layer, Blend and Animation contribute"
        );
    }
    assert_eq!(
        standalone.pointer("/animations/0/precedingBlend/state"),
        Some(&json!("absent")),
        "Blend declarations do not cross a layer boundary"
    );
    Ok(())
}

fn assert_flags(layer: &Value, standalone: &Value) -> TestResult {
    let duplicate_flags = layer
        .pointer("/animations/0/flags")
        .ok_or("two authored flag attributes")?;
    assert_selector(duplicate_flags, "invalid", &["Loop", ""])?;
    assert_distinct_attribute_positions(duplicate_flags)?;
    assert_selector(
        layer
            .pointer("/animations/4/flags")
            .ok_or("repeated flag tokens")?,
        "present",
        &["Loop Loop"],
    )?;
    assert_selector(
        standalone
            .pointer("/animations/0/flags")
            .ok_or("empty flags")?,
        "present",
        &[""],
    )?;
    Ok(())
}

#[test]
fn text_adb_sibling_relation_records_original_order_without_a_runtime_binding() -> TestResult {
    let root = r#"<SubADBs><SubADB File="Animations/test/layers.adb"/></SubADBs>"#;
    let bytes = br#"<AnimDB><FragmentList><Idle><Fragment><AnimLayer>
      <Animation name="Early"/><Blend Duration="002.500"/>
      <Other/><Animation name="Later" flags=" Loop "/><Blend/>
      </AnimLayer></Fragment></Idle></FragmentList></AnimDB>"#;
    let (_temp, archive) = fixture_with(root, [TestEntry::stored(BRANCH, bytes)])?;
    let mut documents = Documents::default();
    let capture = serde_json::to_value(PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male],
        &CancelToken::new(),
        &mut documents,
    )?)?;
    let (index, _, _) = documents
        .0
        .iter()
        .find(|(_, _, stored)| stored == bytes)
        .ok_or("one exact text ADB source")?;
    let layer = capture["adbDeclarations"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.get("document") == Some(&json!(index)))
        })
        .and_then(|row| row.pointer("/fragments/0/layers/0"))
        .ok_or("source layer")?;
    assert_text_layer(layer);
    Ok(())
}

fn assert_text_layer(layer: &Value) {
    assert_eq!(
        layer.pointer("/animations/0/precedingBlend/state"),
        Some(&json!("absent")),
        "no following Blend may become a preceding occurrence"
    );
    assert_eq!(
        layer.pointer("/animations/1/precedingBlend/value"),
        Some(&json!(1)),
        "intervening other node does not erase the preceding declaration"
    );
    assert_eq!(
        layer.pointer("/animations/1/precedingBlend/containsEncryptedSource"),
        Some(&json!(false)),
        "plain source stays plain"
    );
    assert_eq!(
        layer.pointer("/animations/1/flags/occurrences/0/value/value"),
        Some(&json!(" Loop ")),
        "flag text is preserved verbatim"
    );
    assert_eq!(
        layer.pointer("/blends/0/duration/occurrences/0/value/value"),
        Some(&json!("002.500")),
        "original lexical duration is preserved"
    );
    assert_eq!(
        layer.pointer("/blends/0/duration/occurrences/0/position/type"),
        Some(&json!("text")),
        "text field has original byte coordinates"
    );
    assert_eq!(
        layer.pointer("/blends/1/ordinal"),
        Some(&json!(4)),
        "unreferenced later Blend remains visible"
    );
}
