// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::{DataForge, TagBuildEvent, TagProjectionFailure, TagRelationIssue, TagTree};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const FIRST: u128 = 0x6900_0000_0000_4000_8000_0000_0000_0001;

#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Metadata(u32),
    Read(u32, bool),
    Duplicate(u32),
    Projected(u32),
    Failed(u32, TagProjectionFailure),
    Relation {
        record_index: u32,
        target: Option<crate::Guid>,
        reason: TagRelationIssue,
    },
    Cancelled,
}

fn capture(seen: &mut Vec<Seen>, event: &TagBuildEvent<'_, '_>) {
    seen.push(match event {
        TagBuildEvent::MetadataFailed { record, .. } => Seen::Metadata(record.index()),
        TagBuildEvent::CanonicalRead { record, outcome } => {
            Seen::Read(record.index(), outcome.is_ok())
        }
        TagBuildEvent::DuplicateSkipped { record } => Seen::Duplicate(record.index()),
        TagBuildEvent::Projected { record } => Seen::Projected(record.index()),
        TagBuildEvent::ProjectionFailed { record, reason } => Seen::Failed(record.index(), *reason),
        TagBuildEvent::RelationSkipped {
            record,
            target,
            reason,
            ..
        } => Seen::Relation {
            record_index: record.index(),
            target: *target,
            reason: *reason,
        },
        TagBuildEvent::Cancelled => Seen::Cancelled,
    });
}

fn builder() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::Reference),
        )
        .structure(TestStruct::new("TagDatabase").array("tags", TestType::Reference))
}

fn tag(guid: u128, name: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), "synthetic/tags.xml")
        .value("tagName", TestValue::text(name))
        .value(
            "children",
            TestValue::Array(children.iter().copied().map(TestValue::Reference).collect()),
        )
}

fn observed(bytes: &[u8]) -> Result<(Vec<Seen>, TagTree), Box<dyn std::error::Error>> {
    let forge = DataForge::open(bytes)?;
    let mut seen = Vec::new();
    let tree = TagTree::build_observed(&forge, &|| false, &mut |event| {
        capture(&mut seen, &event);
        Ok::<_, Infallible>(())
    })?
    .ok_or("unexpected cancellation")?;
    let legacy = TagTree::build(&forge);
    let paths = |tree: &TagTree| {
        tree.iter()
            .map(|tag| (tag.guid(), tag.path().to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        paths(&tree),
        paths(&legacy),
        "observation preserves the existing tag projection"
    );
    assert_eq!(
        tree.issues(),
        legacy.issues(),
        "observation preserves existing issue counters"
    );
    Ok((seen, tree))
}

#[test]
fn successful_events_report_each_physical_read_once_before_projection() -> TestResult {
    let bytes = builder().record(tag(FIRST, "root", &[])).build();
    let (seen, tree) = observed(&bytes)?;
    assert_eq!(tree.len(), 1, "the readable tag was projected");
    assert_eq!(
        seen,
        [Seen::Read(0, true), Seen::Projected(0)],
        "one canonical read precedes the one actual projection"
    );
    Ok(())
}

#[test]
fn duplicate_guids_keep_distinct_physical_read_and_skip_events() -> TestResult {
    let bytes = builder()
        .record(tag(FIRST, "first", &[]))
        .record(tag(FIRST, "second", &[]))
        .build();
    let (seen, tree) = observed(&bytes)?;
    assert_eq!(tree.len(), 1, "the first readable physical Record wins");
    assert_eq!(
        tree.iter().next().ok_or("tag missing")?.name(),
        "first",
        "duplicate handling preserves the original winner"
    );
    assert!(
        seen.contains(&Seen::Read(0, true)) && seen.contains(&Seen::Read(1, true)),
        "both physical Records were actually read"
    );
    assert!(
        seen.contains(&Seen::Duplicate(1)),
        "the later physical source is intentionally skipped"
    );
    assert!(
        !seen.contains(&Seen::Projected(1)),
        "a duplicate skip is not fabricated as a second projection"
    );
    Ok(())
}

#[test]
fn rejected_names_and_their_descendants_have_typed_limit_events() -> TestResult {
    let bytes = builder()
        .record(tag(
            FIRST,
            &"x".repeat(crate::MAX_TAG_NAME_BYTES + 1),
            &[FIRST + 1],
        ))
        .record(tag(FIRST + 1, "child", &[]))
        .build();
    let (seen, tree) = observed(&bytes)?;
    assert!(
        tree.is_empty(),
        "a rejected ancestor and its descendants stay outside the tag tree"
    );
    assert!(
        seen.contains(&Seen::Failed(0, TagProjectionFailure::NameBytes)),
        "the actual name limit is identified"
    );
    assert!(
        seen.contains(&Seen::Failed(1, TagProjectionFailure::AncestorDiscarded)),
        "descendant omission has a separate actual source outcome"
    );
    Ok(())
}

#[test]
fn path_and_depth_limits_report_the_exact_rejected_source() -> TestResult {
    let mut spec = builder();
    for index in 0_u32..7 {
        let children = if index < 6 {
            vec![FIRST + u128::from(index + 1)]
        } else {
            Vec::new()
        };
        spec = spec.record(tag(FIRST + u128::from(index), &"x".repeat(200), &children));
    }
    let (seen, _) = observed(&spec.build())?;
    assert!(
        seen.contains(&Seen::Failed(5, TagProjectionFailure::PathBytes)),
        "the sixth path exceeds the existing byte limit"
    );
    let mut spec = builder();
    for index in 0_u32..66 {
        let children = if index < 65 {
            vec![FIRST + u128::from(index + 1)]
        } else {
            Vec::new()
        };
        spec = spec.record(tag(FIRST + u128::from(index), "x", &children));
    }
    let (seen, _) = observed(&spec.build())?;
    assert!(
        seen.contains(&Seen::Failed(64, TagProjectionFailure::Depth)),
        "the first out-of-depth source is retained"
    );
    assert!(
        seen.contains(&Seen::Failed(65, TagProjectionFailure::AncestorDiscarded)),
        "the descendant of an overdeep source is retained"
    );
    Ok(())
}

#[test]
fn failed_canonical_reads_are_not_projection_failures() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(TestRecord::new(
            FIRST,
            "Tag",
            "Tag.Synthetic",
            "synthetic/tags.xml",
        ))
        .build();
    let (seen, tree) = observed(&bytes)?;
    assert!(
        tree.is_empty(),
        "an unreadable source cannot produce a Tag node"
    );
    assert_eq!(
        seen,
        [Seen::Read(0, false)],
        "the canonical failure is reported before any projection can occur"
    );
    Ok(())
}

#[test]
fn malformed_metadata_is_reported_without_guessing_a_tag_type() -> TestResult {
    let mut bytes = builder().record(tag(FIRST, "root", &[])).build();
    let first_name = crate::header::SNIFF_LEN;
    let name = bytes
        .get_mut(first_name..first_name + u32::MAX.to_le_bytes().len())
        .ok_or("synthetic struct name is absent")?;
    name.copy_from_slice(&u32::MAX.to_le_bytes());
    let forge = DataForge::open(&bytes)?;
    assert!(
        matches!(
            forge.record(0).ok_or("source absent")?.type_name(),
            Err(crate::DataForgeError::InvalidString { .. })
        ),
        "the fixture opens successfully and fails only at the actual lazy type-name boundary"
    );
    let (seen, tree) = observed(&bytes)?;
    assert!(
        tree.is_empty(),
        "unreadable type metadata cannot establish a Tag projection"
    );
    assert_eq!(
        seen,
        [Seen::Metadata(0)],
        "unreadable type metadata is a physical selection-candidate observation without canonical decode"
    );
    Ok(())
}

#[test]
fn relation_events_do_not_change_the_source_projection_disposition() -> TestResult {
    let bytes = builder()
        .record(tag(FIRST, "root", &[FIRST, FIRST + 1]))
        .build();
    let (seen, tree) = observed(&bytes)?;
    let root: crate::Guid = "69000000-0000-4000-8000-000000000001".parse()?;
    let missing: crate::Guid = "69000000-0000-4000-8000-000000000002".parse()?;
    assert_eq!(
        tree.len(),
        1,
        "the readable root remains projected despite unusable child relations"
    );
    assert!(
        seen.contains(&Seen::Relation {
            record_index: 0,
            target: Some(root),
            reason: TagRelationIssue::Repeated
        }),
        "cycles are reported as repeated relations"
    );
    assert!(
        seen.contains(&Seen::Relation {
            record_index: 0,
            target: Some(missing),
            reason: TagRelationIssue::Dangling
        }),
        "missing targets remain typed relation evidence"
    );
    assert!(
        seen.contains(&Seen::Projected(0)),
        "relation diagnostics never replace the successful source projection"
    );
    Ok(())
}

#[test]
fn observer_errors_abort_without_an_extra_decode_or_completed_projection() -> TestResult {
    let bytes = builder().record(tag(FIRST, "root", &[])).build();
    let forge = DataForge::open(&bytes)?;
    let mut reads = 0;
    let outcome = TagTree::build_observed(&forge, &|| false, &mut |event| {
        if matches!(event, TagBuildEvent::CanonicalRead { .. }) {
            reads += 1;
        }
        Err::<(), _>("observer rejected the source")
    });
    assert_eq!(
        outcome.err(),
        Some("observer rejected the source"),
        "the exact observer error propagates"
    );
    assert_eq!(
        reads, 1,
        "the source is decoded exactly once before observer rejection"
    );
    Ok(())
}

#[test]
fn cancellation_is_reported_explicitly_without_a_completed_tree() -> TestResult {
    let bytes = builder().record(tag(FIRST, "root", &[])).build();
    let forge = DataForge::open(&bytes)?;
    let mut seen = Vec::new();
    let tree = TagTree::build_observed(&forge, &|| true, &mut |event| {
        capture(&mut seen, &event);
        Ok::<_, Infallible>(())
    })?;
    assert!(
        tree.is_none(),
        "cancellation does not publish a partial tree"
    );
    assert_eq!(
        seen,
        [Seen::Cancelled],
        "cancellation is observed without a source read"
    );
    Ok(())
}
