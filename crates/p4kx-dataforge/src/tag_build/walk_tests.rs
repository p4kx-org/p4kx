// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::convert::Infallible;
use std::error::Error;

use crate::{DataForge, MAX_TAG_TEXT_BYTES, TagBuildEvent, TagProjectionFailure, TagTreeIssues};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::super::Ticker;
use super::{Raw, Walk};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn cumulative_text_budget_cuts_the_actual_next_node_and_emits_no_chosen_path() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .record(
            TestRecord::new(1, "Tag", "Tag.Synthetic", "synthetic/tags.xml")
                .value("tagName", TestValue::text("x")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source")?;
    let raws = [Raw {
        record,
        guid: record.guid(),
        name: "x",
        legacy_id: None,
        children: Vec::new(),
    }];
    let positions = HashMap::from([(record.guid(), 0)]);
    let stop = || false;
    let mut walk = Walk {
        raws: &raws,
        positions: &positions,
        visited: vec![false],
        nodes: Vec::new(),
        roots: Vec::new(),
        text_bytes: MAX_TAG_TEXT_BYTES,
        issues: TagTreeIssues::default(),
        ticker: Ticker::new(&stop),
    };
    let mut cut = None;
    let mut paths = 0;
    walk.root(0, false, None, &mut |event| {
        match event {
            TagBuildEvent::ProjectionFailed { record, reason } => {
                cut = Some((record.index(), reason));
            }
            TagBuildEvent::PathChosen { .. } => paths += 1,
            _ => {}
        }
        Ok::<_, Infallible>(())
    })?;
    assert_eq!(cut, Some((0, TagProjectionFailure::TextBytes)));
    assert_eq!(paths, 0);
    assert!(
        walk.nodes.is_empty(),
        "a cumulative-text cut does not create a convenience node"
    );
    assert_eq!(
        walk.text_bytes, MAX_TAG_TEXT_BYTES,
        "no budget overrun or dropped accounting"
    );
    assert_eq!(
        walk.visited,
        [true],
        "discarded source is not re-rooted later"
    );
    Ok(())
}
