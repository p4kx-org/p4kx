// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Soundbank;
use p4kx_extraction::hirc_view::ObjectView;
use p4kx_testkit::{TestBank, TestHircObject, TestNodeBase};
use serde_json::{Value, json};

#[test]
fn actor_mixer_view_exposes_physical_relationships_and_exact_bank_spans()
-> Result<(), Box<dyn Error>> {
    let children = [37_u32, 19, 37];
    let bytes = TestBank::new(12)
        .object(TestHircObject::actor_mixer(
            1,
            &children,
            TestNodeBase::Full,
        ))
        .bytes();
    let bank = Soundbank::parse(&bytes)?;
    let actor = bank.objects().first().ok_or("original ActorMixer")?;
    let view = serde_json::to_value(ObjectView::new(0, actor))?;
    assert_eq!(
        view.get("type"),
        Some(&json!("actor-mixer")),
        "original type"
    );
    assert_eq!(
        view.get("status"),
        Some(&json!("decoded")),
        "complete supported layout"
    );
    assert_eq!(
        view.get("parent"),
        Some(&json!(0x2C21_1B2E)),
        "original nonzero parent"
    );
    assert_eq!(
        view.get("children"),
        Some(&json!(children)),
        "original child order with repeats"
    );
    let detail = view.get("detail").ok_or("physical spans")?;
    let node = read_range(detail, "node_range")?;
    let count = read_range(detail, "child_count_range")?;
    let words = read_range(detail, "children_range")?;
    assert_eq!(
        node.end, count.start,
        "source count follows the shared node base"
    );
    assert_eq!(count.end, words.start, "source children follow the count");
    assert_eq!(
        bytes.get(count),
        Some(3_u32.to_le_bytes().as_slice()),
        "original child-count bytes"
    );
    assert_eq!(
        bytes.get(words),
        Some(
            children
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
                .as_slice()
        ),
        "the child list range selects the original words"
    );
    Ok(())
}

fn read_range(value: &Value, field: &str) -> Result<std::ops::Range<usize>, Box<dyn Error>> {
    let range = value.get(field).ok_or("declared bank-relative span")?;
    let offset = |edge| -> Result<usize, Box<dyn Error>> {
        Ok(range
            .get(edge)
            .and_then(Value::as_str)
            .ok_or("decimal range offset")?
            .parse()?)
    };
    Ok(offset("start")?..offset("end_exclusive")?)
}
