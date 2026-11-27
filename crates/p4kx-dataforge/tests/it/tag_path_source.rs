// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::convert::Infallible;
use std::error::Error;

use p4kx_dataforge::{DataForge, TagBuildEvent, TagPathEdgeKind, TagTree};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Debug, PartialEq, Eq)]
struct Chosen {
    source: u32,
    position: u32,
    parent: Option<u32>,
    incoming: Option<(u32, TagPathEdgeKind, usize)>,
    path: String,
    depth: u32,
    listed: bool,
}

fn tag(id: u128, name: &str, children: Vec<TestValue>) -> TestRecord {
    TestRecord::new(id, "Tag", format!("Tag.{name}"), "synthetic/tags.xml")
        .value("tagName", TestValue::text(name))
        .value("children", TestValue::Array(children))
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(TestStruct::new("Other"))
        .record(tag(
            1,
            "Root",
            vec![
                TestValue::Null,
                TestValue::Reference(7),
                TestValue::Reference(99),
                TestValue::Reference(2),
                TestValue::Reference(2),
            ],
        ))
        .record(tag(2, "Same", vec![TestValue::Reference(1)]))
        .record(tag(3, "Same", vec![TestValue::Reference(2)]))
        .record(tag(4, "Loose", Vec::new()))
        .record(
            TestRecord::new(5, "TagDatabase", "TagDatabase.Roots", "synthetic/roots.xml").value(
                "tags",
                TestValue::Array(vec![
                    TestValue::Null,
                    TestValue::Reference(7),
                    TestValue::Reference(1),
                    TestValue::Reference(3),
                    TestValue::Reference(1),
                ]),
            ),
        )
        .record(tag(2, "Shadow", Vec::new()))
        .record(TestRecord::new(
            7,
            "Other",
            "Other.Wrong",
            "synthetic/other.xml",
        ))
        .build()
}

fn chosen(event: &TagBuildEvent<'_, '_>) -> Option<Chosen> {
    let TagBuildEvent::PathChosen {
        record,
        position,
        parent_position,
        incoming,
        path,
        depth,
        listed,
    } = event
    else {
        return None;
    };
    Some(Chosen {
        source: record.index(),
        position: *position,
        parent: *parent_position,
        incoming: incoming.map(|edge| (edge.record().index(), edge.kind(), edge.ordinal())),
        path: path.to_string(),
        depth: *depth,
        listed: *listed,
    })
}

fn expected_trace() -> [Chosen; 4] {
    [
        Chosen {
            source: 0,
            position: 0,
            parent: None,
            incoming: Some((4, TagPathEdgeKind::Root, 2)),
            path: "Root".to_owned(),
            depth: 0,
            listed: true,
        },
        Chosen {
            source: 1,
            position: 1,
            parent: Some(0),
            incoming: Some((0, TagPathEdgeKind::Child, 3)),
            path: "Root.Same".to_owned(),
            depth: 1,
            listed: true,
        },
        Chosen {
            source: 2,
            position: 2,
            parent: None,
            incoming: Some((4, TagPathEdgeKind::Root, 3)),
            path: "Same".to_owned(),
            depth: 0,
            listed: true,
        },
        Chosen {
            source: 3,
            position: 3,
            parent: None,
            incoming: None,
            path: "Loose".to_owned(),
            depth: 0,
            listed: false,
        },
    ]
}

#[test]
fn actual_trace_preserves_original_slots_and_first_selected_physical_parent() -> TestResult {
    let bytes = bytes();
    let forge = DataForge::open(&bytes)?;
    let mut seen = Vec::new();
    let mut reads = Vec::new();
    let tree = TagTree::build_observed(&forge, &|| false, &mut |event| {
        if let TagBuildEvent::CanonicalRead { record, .. } = &event {
            reads.push(record.index());
        }
        seen.extend(chosen(&event));
        Ok::<_, Infallible>(())
    })?
    .ok_or("tree cancelled")?;
    assert_eq!(
        reads,
        [0, 1, 2, 3, 4, 5],
        "one canonical read per physical Tag/Database including duplicate"
    );
    assert_eq!(seen, expected_trace());
    let plain = TagTree::build(&forge);
    let paths = |tree: &TagTree| {
        tree.iter()
            .map(|tag| (tag.guid(), tag.path().to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        paths(&tree),
        paths(&plain),
        "trace never selects a different graph"
    );
    assert_eq!(
        tree.issues(),
        plain.issues(),
        "trace never changes graph counters"
    );
    assert_eq!(TagPathEdgeKind::Root.field(), "tags");
    assert_eq!(TagPathEdgeKind::Child.field(), "children");
    Ok(())
}

#[test]
fn rejecting_the_first_trace_event_stops_before_later_paths_without_rereading_sources() -> TestResult
{
    let bytes = bytes();
    let forge = DataForge::open(&bytes)?;
    let reads = Cell::new(0);
    let paths = Cell::new(0);
    let result = TagTree::build_observed(&forge, &|| false, &mut |event| {
        if matches!(event, TagBuildEvent::CanonicalRead { .. }) {
            reads.set(reads.get() + 1);
        }
        if matches!(event, TagBuildEvent::PathChosen { .. }) {
            paths.set(paths.get() + 1);
            return Err("trace cancelled");
        }
        Ok(())
    });
    assert_eq!(result.err(), Some("trace cancelled"));
    assert_eq!(
        paths.get(),
        1,
        "no later trace callback runs after rejection"
    );
    assert_eq!(
        reads.get(),
        6,
        "original single canonical pass was not replayed"
    );
    Ok(())
}
