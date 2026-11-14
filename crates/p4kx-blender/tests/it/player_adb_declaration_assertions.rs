// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{PlayerDocument, PlayerDocumentSink, PlayerSource, PlayerSourceCapture};
use p4kx_extraction::CancelToken;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::TestResult;

#[path = "player_adb_blend_assertions.rs"]
mod blends;

const BRANCH: &str = "Data/Animations/test/layers.adb";

#[derive(Default)]
struct Documents(Vec<(usize, Value, Vec<u8>)>);

impl PlayerDocumentSink for Documents {
    fn document(&mut self, document: PlayerDocument<'_>) -> io::Result<()> {
        self.0.push((
            document.index,
            serde_json::to_value(document.source)?,
            document.bytes.to_vec(),
        ));
        Ok(())
    }
}

fn branch() -> Vec<u8> {
    let layer = TestElement::new("AnimLayer")
        .child(
            TestElement::new("Blend")
                .attribute("Duration", "1")
                .attribute("StartTime", "")
                .attribute("ExitTime", ".5"),
        )
        .child(
            TestElement::new("Animation")
                .attribute("name", "same")
                .attribute("name", "second")
                .attribute("flags", "Loop")
                .attribute("flags", ""),
        )
        .child(TestElement::new("Animation").attribute("name", ""))
        .child(TestElement::new("Animation").attribute("Name", "opaque"))
        .child(
            TestElement::new("Animation")
                .attribute("name", "same")
                .attribute("frames", "authored"),
        )
        .child(TestElement::new("Blend").attribute("Duration", "0"))
        .child(TestElement::new("Other"))
        .child(TestElement::new("Animation").attribute("flags", "Loop Loop"));
    TestElement::new("AnimDB")
        .child(TestElement::new("Unknown"))
        .child(
            TestElement::new("FragmentList").child(
                TestElement::new("Walk")
                    .child(TestElement::new("Other"))
                    .child(
                        TestElement::new("Fragment")
                            .attribute("GUID", "raw-a")
                            .attribute("GUID", "raw-b")
                            .child(TestElement::new("ProcLayer"))
                            .child(layer)
                            .child(TestElement::new("AnimLayer"))
                            .child(
                                TestElement::new("AnimLayer")
                                    .child(TestElement::new("Animation").attribute("flags", "")),
                            ),
                    ),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

#[test]
fn authored_adb_layers_keep_exact_source_positions_without_renumbering_legacy_uses() -> TestResult {
    let root = r#"<SubADBs>
        <SubADB File="Animations/test/layers.adb"/>
        <SubADB File="DATA/ANIMATIONS/TEST/LAYERS.ADB"/>
        </SubADBs>"#;
    let bytes = branch();
    let (_temp, archive) = super::fixture_with(
        root,
        [TestEntry::stored(BRANCH, bytes.as_slice()).encrypted()],
    )?;
    let mut documents = Documents::default();
    let cancel = CancelToken::new();
    let v1 = serde_json::to_value(PlayerSource::read(&archive, BodyVariant::Male, &cancel)?)?;
    let capture = serde_json::to_value(PlayerSourceCapture::read(
        &archive,
        &[BodyVariant::Male],
        &cancel,
        &mut documents,
    )?)?;
    super::shared_capture::same_legacy_inventory(&v1, &capture)?;
    assert_eq!(
        capture.get("motion"),
        v1.get("motion"),
        "same legacy motion and references"
    );
    let (index, source, original) = documents
        .0
        .iter()
        .find(|(_, _, original)| *original == bytes)
        .ok_or("one exact verified original")?;
    assert_eq!(
        original, &bytes,
        "preserve entire original including unknown declarations"
    );
    let adb = capture
        .get("adbDeclarations")
        .and_then(Value::as_array)
        .ok_or("ADB sources")?;
    assert_eq!(adb.len(), 2, "root and one physical child despite alias");
    let row = adb
        .iter()
        .find(|row| row.get("document") == Some(&json!(index)))
        .ok_or("bound ADB")?;
    assert_eq!(
        row.get("source"),
        Some(source),
        "same source as exact original"
    );
    let root = adb
        .iter()
        .find(|row| row.get("document") != Some(&json!(index)))
        .ok_or("root ADB")?;
    assert_eq!(
        root.pointer("/fragments/0/position/value/type"),
        Some(&json!("text")),
        "root text position"
    );
    assert_fragment(row, &capture, source)?;
    Ok(())
}

fn assert_fragment(adb: &Value, capture: &Value, source: &Value) -> TestResult {
    let option = adb.pointer("/fragments/0").ok_or("authored Fragment")?;
    for (key, ordinal) in [
        ("listOrdinal", 1),
        ("groupOrdinal", 0),
        ("optionOrdinal", 1),
    ] {
        assert_eq!(
            option.get(key),
            Some(&json!(ordinal)),
            "{key} counts all direct elements"
        );
    }
    assert_eq!(
        option.pointer("/name/value"),
        Some(&json!("Walk")),
        "exact group spelling"
    );
    assert_eq!(
        option.pointer("/groupPosition/value/node"),
        Some(&json!(3)),
        "group node in CryXml table"
    );
    assert_eq!(
        option.pointer("/position/value/node"),
        Some(&json!(5)),
        "option node in CryXml table"
    );
    let guid = option.get("guid").ok_or("raw GUID")?;
    assert_selector(guid, "invalid", &["raw-a", "raw-b"])?;
    assert_attribute_evidence(guid, capture, source, [5, 0], "GUID")?;
    assert_distinct_attribute_positions(guid)?;
    let layers = option
        .get("layers")
        .and_then(Value::as_array)
        .ok_or("authored layers")?;
    let [first, empty, standalone] = layers.as_slice() else {
        return Err("three exact layers".into());
    };
    assert_eq!(
        first.get("ordinal"),
        Some(&json!(1)),
        "procedural sibling remains in source ordinal"
    );
    assert_eq!(
        empty.get("ordinal"),
        Some(&json!(2)),
        "second layer keeps original position"
    );
    assert_eq!(
        empty.get("animations"),
        Some(&json!([])),
        "authored empty layer"
    );
    assert_animations(first, capture, source)?;
    blends::assert_blends(first, standalone, capture, source)
}

fn assert_animations(layer: &Value, capture: &Value, source: &Value) -> TestResult {
    let animations = layer
        .get("animations")
        .and_then(Value::as_array)
        .ok_or("layer declarations")?;
    let [duplicate, empty, absent, repeated, trailing] = animations.as_slice() else {
        return Err("five Animations".into());
    };
    for (row, ordinal, state, names) in [
        (duplicate, 1, "invalid", &["same", "second"][..]),
        (empty, 2, "present", &[""][..]),
        (absent, 3, "absent", &[][..]),
        (repeated, 4, "present", &["same"][..]),
        (trailing, 7, "absent", &[][..]),
    ] {
        assert_eq!(
            row.get("ordinal"),
            Some(&json!(ordinal)),
            "position among all sibling elements"
        );
        let selected = row.get("name").ok_or("authored name selector")?;
        assert_selector(selected, state, names)?;
    }
    let name = duplicate.get("name").ok_or("repeated name declaration")?;
    assert_distinct_attribute_positions(name)?;
    let first_attr = name
        .pointer("/occurrences/0/position/attribute")
        .and_then(Value::as_u64)
        .ok_or("name table position")?;
    assert_attribute_evidence(name, capture, source, [9, first_attr], "name")
}

fn assert_selector(selector: &Value, state: &str, expected: &[&str]) -> TestResult {
    assert_eq!(
        selector.get("state"),
        Some(&json!(state)),
        "selector state independent of content"
    );
    assert_eq!(
        selector.get("containsEncryptedSource"),
        Some(&json!(true)),
        "selected branch is encrypted"
    );
    assert_eq!(
        selector.get("sourceSetComplete"),
        Some(&json!(true)),
        "full observed set"
    );
    let occurrences = selector
        .get("occurrences")
        .and_then(Value::as_array)
        .ok_or("original occurrences")?;
    assert_eq!(occurrences.len(), expected.len(), "no silent collapse");
    for (row, text) in occurrences.iter().zip(expected) {
        assert_eq!(
            row.pointer("/value/value"),
            Some(&json!(text)),
            "original attribute text"
        );
    }
    Ok(())
}

fn assert_distinct_attribute_positions(selector: &Value) -> TestResult {
    let entries = selector
        .get("occurrences")
        .and_then(Value::as_array)
        .ok_or("raw duplicate coordinates")?;
    let [first, second] = entries.as_slice() else {
        return Err("two raw duplicate occurrences".into());
    };
    assert_ne!(
        first.get("position"),
        second.get("position"),
        "original attribute rows remain separate"
    );
    assert_ne!(
        first.pointer("/value/sourceUsesRefs"),
        second.pointer("/value/sourceUsesRefs"),
        "independent physical receipts"
    );
    Ok(())
}

fn assert_attribute_evidence(
    selector: &Value,
    capture: &Value,
    source: &Value,
    [node, attribute]: [u64; 2],
    field: &str,
) -> TestResult {
    let original = selector
        .pointer("/occurrences/0")
        .ok_or("positioned attribute")?;
    assert_eq!(
        original.pointer("/position/node"),
        Some(&json!(node)),
        "physical node"
    );
    assert_eq!(
        original.pointer("/position/attribute"),
        Some(&json!(attribute)),
        "physical attribute"
    );
    let refs = original
        .pointer("/value/sourceUsesRefs/0")
        .ok_or("own field use")?;
    let used = capture
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .and_then(|rows| rows.iter().find(|row| row.get("id") == Some(refs)))
        .ok_or("bound source use")?;
    assert_eq!(
        used.get("source"),
        Some(source),
        "same admitted physical ADB"
    );
    assert_eq!(
        used.pointer("/location/field"),
        Some(&json!(field)),
        "positioned original field"
    );
    assert_eq!(
        used.pointer("/location/indices"),
        Some(&json!([node.to_string(), attribute.to_string()])),
        "exact CryXml coordinate"
    );
    Ok(())
}
