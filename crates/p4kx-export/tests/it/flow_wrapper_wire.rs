// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole};
use serde_json::Value;

use crate::flow_final_origin;
use crate::flow_owner_graph::{self, Owner};
use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph;

use super::fixture;

pub(super) struct Evidence<'a> {
    pub(super) inventory: &'a SourceInventory,
    pub(super) definition: &'a Value,
    pub(super) kind: Input,
}

pub(super) fn root<'catalog>(
    owner: &Owner<'catalog>,
    triggers: &[Value],
) -> TestResult<&'catalog Value> {
    let [trigger] = triggers else {
        return Err("one original trigger".into());
    };
    let trigger = owner.follow(trigger)?;
    owner.follow(
        trigger
            .pointer("/trigger/action")
            .ok_or("original root action")?,
    )
}

pub(super) fn meaning<'node>(node: &'node Value, expected: &str) -> TestResult<&'node Value> {
    let meaning = node.get("action").ok_or("action meaning")?;
    assert_eq!(meaning["type"], expected, "actual typed action");
    Ok(meaning)
}

pub(super) fn check_mixed(
    owner: &Owner<'_>,
    root: &Value,
    evidence: &Evidence<'_>,
    expected: &str,
) -> TestResult {
    let wrapper = meaning(root, "multipleActions")?;
    let actions = raw::array(wrapper, "actions")?;
    let original = template_canonical_graph::struct_field(owner, root, "actions")?;
    assert_eq!(
        actions,
        raw::elements(original)?,
        "all actual action slots in authored order"
    );
    let [first, null, pick, last] = actions else {
        return Err("original mixed wrapper slots".into());
    };
    assert_ne!(first, last, "equal leaves keep distinct source identity");
    meaning(owner.follow(first)?, "stateChange")?;
    raw::source_null(owner.follow(null)?);
    check_pick(owner, owner.follow(pick)?, evidence, expected)?;
    meaning(owner.follow(last)?, "stateChange")?;
    Ok(())
}

fn check_pick(
    owner: &Owner<'_>,
    node: &Value,
    evidence: &Evidence<'_>,
    expected: &str,
) -> TestResult {
    let entries = raw::array(meaning(node, "pickRandomAction")?, "entries")?;
    let originals = raw::elements(template_canonical_graph::struct_field(
        owner, node, "actions",
    )?)?;
    assert_eq!(
        entries.len(),
        originals.len(),
        "random entries preserve count"
    );
    for (entry, reference) in entries.iter().zip(originals) {
        assert_eq!(
            entry["type"], "declared",
            "authored random action entry type"
        );
        assert_eq!(
            entry.get("value"),
            Some(reference),
            "entry binds original Class array slot"
        );
    }
    let [token, null, empty] = entries else {
        return Err("three original weighted entries".into());
    };
    weight(owner, token, evidence, -0.0, "finite")?;
    weight(
        owner,
        null,
        evidence,
        f32::from_bits(fixture::NAN),
        "nonfinite",
    )?;
    weight(owner, empty, evidence, 0.75, "finite")?;
    objective(owner, owner.follow(&token["action"])?, expected)?;
    raw::source_null(owner.follow(&null["action"])?);
    let empty = meaning(owner.follow(&empty["action"])?, "multipleActions")?;
    assert_eq!(
        empty.pointer("/actionsField/type").and_then(Value::as_str),
        Some("field"),
        "empty has a real declaration"
    );
    assert!(
        raw::array(empty, "actions")?.is_empty(),
        "authored empty action array"
    );
    Ok(())
}

pub(super) fn objective(owner: &Owner<'_>, action: &Value, expected: &str) -> TestResult {
    let binding = &meaning(action, "tokenStateChange")?["objective"];
    assert_eq!(
        binding["interpretation"], "id",
        "nested leaf resolves its own original objective"
    );
    let field = owner.field(&binding["field"])?;
    let value = owner.follow(&field["value"])?;
    assert_eq!(
        raw::payload(value, "guid")?["value"],
        expected,
        "physical owner-local target"
    );
    Ok(())
}

fn weight(
    owner: &Owner<'_>,
    entry: &Value,
    evidence: &Evidence<'_>,
    expected: f32,
    interpretation: &str,
) -> TestResult {
    assert_eq!(
        entry["probabilityInterpretation"], interpretation,
        "authored probability interpretation"
    );
    let slot = owner.follow(&entry["value"])?;
    let source = template_canonical_graph::struct_field(owner, slot, "probability")?;
    let number = &raw::payload(source, "number")?["value"];
    assert_eq!(number["type"], "f32", "authored Single storage");
    assert_eq!(
        number["rawBits"],
        format!("{:08x}", expected.to_bits()),
        "original authored weight"
    );
    assert!(
        entry.get("probability").is_none(),
        "semantic binding references the one original datum"
    );
    let input = evidence.definition["inputRef"]
        .as_str()
        .ok_or("physical source input")?;
    let owner = evidence
        .definition
        .pointer("/recordIndex/value")
        .and_then(Value::as_u64)
        .ok_or("physical owner position")?;
    let location = SourceLocation::Subresource {
        namespace: "dataforgeValue".to_owned(),
        indices: vec![owner, flow_owner_graph::ordinal(source)?],
        field: "$value".to_owned(),
    };
    flow_final_origin::closed(
        &source["value"],
        evidence.inventory,
        evidence.kind.marker(),
        true,
        input,
        &[(SourceUseRole::Payload, location)],
    )
}
