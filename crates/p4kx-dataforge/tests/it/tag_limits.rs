// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Tag source boundaries and duplicate GUIDs over synthetic DataForges.

use std::error::Error;

use p4kx_dataforge::{DataForge, TagTree};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const DATABASE: u128 = 0x0d00_0000_0000_4000_8000_0000_0000_0001;
const PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";

const fn guid_of(index: usize) -> u128 {
    0x00a0_0000_0000_4000_8000_0000_0000_0000 + index as u128
}

fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().map(|g| TestValue::Reference(*g)).collect())
}

fn tag(guid: u128, name: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{guid:x}"), PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(1))
        .value("children", references(children))
}

fn builder(roots: &[u128]) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".into())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".into())),
        )
        .record(
            TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", PATH)
                .value("tags", references(roots)),
        )
}

/// Returns a DataForge with a chain of `length` tags, each the only child of the one before, and
/// the name of tag `index` from `name`.
fn chain(length: usize, name: impl Fn(usize) -> String) -> Vec<u8> {
    let mut builder = builder(&[guid_of(0)]);
    for index in 0..length {
        let children = if index + 1 < length {
            vec![guid_of(index + 1)]
        } else {
            Vec::new()
        };
        builder = builder.record(tag(guid_of(index), &name(index), &children));
    }
    builder.build()
}

#[test]
fn a_chain_keeps_its_actual_parent_and_deepest_tag() -> TestResult {
    let bytes = chain(70, |index| format!("T{index}"));
    let forge = DataForge::open(&bytes)?;

    let tags = TagTree::build(&forge);

    assert_eq!(tags.len(), 70, "every physical Tag");
    let issues = tags.issues();
    assert_eq!(
        issues.unlisted(),
        0,
        "the tags below it are not roots of their own"
    );
    assert_eq!(tags.roots().count(), 1, "one root");
    let deepest = tags.iter().last().ok_or("no tag")?;
    assert_eq!(deepest.depth(), 69, "the actual deepest tag");
    let tag = tags
        .get(forge_guid(64)?)
        .ok_or("original Tag at depth 64")?;
    assert_eq!(tag.depth(), 64);
    assert_eq!(tag.parent().map(|parent| parent.position()), Some(63));
    Ok(())
}

fn forge_guid(index: usize) -> Result<p4kx_dataforge::Guid, Box<dyn Error>> {
    let text = format!("{:032x}", guid_of(index));
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    );
    Ok(dashed.parse()?)
}

#[test]
fn large_names_in_a_chain_keep_their_full_joined_length() -> TestResult {
    let bytes = chain(40, |_| "x".repeat(64 << 10));
    let forge = DataForge::open(&bytes)?;

    let tags = TagTree::build(&forge);

    assert_eq!(tags.len(), 40);
    let tail = tags.at(39).ok_or("the final physical tag")?;
    assert_eq!(tail.name().len(), 64 << 10);
    assert_eq!(tail.path_view().byte_len(), 40 * (64 << 10) + 39);
    assert_eq!(
        tags.issues().unlisted(),
        0,
        "nothing below it is rooted instead"
    );
    Ok(())
}

#[test]
fn a_long_path_keeps_the_chain_and_exact_spelling() -> TestResult {
    let name = "n".repeat(256);
    let bytes = chain(20, |_| name.clone());
    let forge = DataForge::open(&bytes)?;

    let tags = TagTree::build(&forge);

    assert_eq!(tags.len(), 20);
    assert_eq!(
        tags.issues().unlisted(),
        0,
        "and nothing below it is rooted instead"
    );
    let last = tags.iter().last().ok_or("no tag")?;
    assert_eq!(last.name(), name, "the name is the end of the path");
    let expected = vec![name.as_str(); 20].join(".");
    assert!(
        last.path_view().eq_text(&expected),
        "all names and separators are retained"
    );
    Ok(())
}

#[test]
fn adjacent_name_lengths_keep_their_exact_text() -> TestResult {
    let bytes = builder(&[guid_of(0), guid_of(1)])
        .record(tag(guid_of(0), &"a".repeat(256), &[]))
        .record(tag(guid_of(1), &"b".repeat(257), &[]))
        .build();
    let forge = DataForge::open(&bytes)?;

    let tags = TagTree::build(&forge);

    assert_eq!(tags.len(), 2);
    assert_eq!(
        tags.at(0).ok_or("first physical Tag")?.name(),
        "a".repeat(256)
    );
    assert_eq!(
        tags.at(1).ok_or("second physical Tag")?.name(),
        "b".repeat(257)
    );
    Ok(())
}

#[test]
fn a_repeated_guid_keeps_the_first_tag_and_is_counted() -> TestResult {
    let bytes = builder(&[guid_of(0)])
        .record(tag(guid_of(0), "First", &[]))
        .record(tag(guid_of(0), "Second", &[]))
        .build();
    let forge = DataForge::open(&bytes)?;

    let tags = TagTree::build(&forge);

    assert_eq!(tags.len(), 1, "one tag for the GUID");
    assert_eq!(tags.issues().duplicate(), 1, "the other is counted");
    assert_eq!(
        tags.path_of(forge_guid(0)?),
        Some("First"),
        "the first in definition order"
    );
    assert!(!tags.issues().is_clean());
    Ok(())
}

#[test]
fn the_sdk_retains_physical_depth_above_uint16() -> TestResult {
    let depth = usize::from(u16::MAX) + 1;
    let bytes = chain(depth + 1, |_| "x".to_owned());
    let forge = DataForge::open(&bytes)?;
    let source = forge
        .record(u32::try_from(depth + 1)?)
        .ok_or("original last Tag")?;
    assert_eq!(source.guid(), forge_guid(depth)?);
    assert_eq!(source.tree()?.error_count(), 0);
    let parent_source = forge
        .record(u32::try_from(depth)?)
        .ok_or("original parent Tag")?;
    let parent_tree = parent_source.tree()?;
    let Some(p4kx_dataforge::Value::Array(children)) = parent_tree.field("children") else {
        return Err("original terminal Reference array".into());
    };
    let [p4kx_dataforge::Value::Reference(child)] = children.as_ref() else {
        return Err("one actual terminal Reference".into());
    };
    assert_eq!(child.record_index(), source.index());

    let tags = TagTree::build(&forge);
    let root = tags.roots().next().ok_or("original listed root")?;
    assert_eq!(root.descendants().len(), depth);
    let tail = tags.get(source.guid()).ok_or("the selected last Tag")?;
    assert_eq!(tail.depth(), depth);
    assert_eq!(
        tail.parent().map(|tag| tag.guid()),
        Some(parent_source.guid())
    );
    assert_eq!(tail.path_view().byte_len(), u64::try_from(depth)? * 2 + 1);
    Ok(())
}

#[test]
#[expect(
    deprecated,
    reason = "Verifies the released Tag SDK compatibility surface"
)]
fn released_tag_limit_names_remain_available_without_pruning() {
    assert_eq!(p4kx_dataforge::MAX_TAG_DEPTH, 64);
    assert_eq!(p4kx_dataforge::MAX_TAG_NAME_BYTES, 256);
    assert_eq!(p4kx_dataforge::MAX_TAG_PATH_BYTES, 1024);
    assert_eq!(p4kx_dataforge::MAX_TAG_TEXT_BYTES, 64 << 20);
    let issues = p4kx_dataforge::TagTreeIssues::default();
    assert_eq!(issues.too_deep(), 0);
    assert_eq!(issues.too_long(), 0);
}

#[path = "tag_default_source.rs"]
mod default_source;
