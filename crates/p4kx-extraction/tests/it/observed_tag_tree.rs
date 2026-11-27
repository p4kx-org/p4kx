// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::sync::Arc;

use p4kx_extraction::{
    DataForgeEntry, ObservedTagTree, SourceEvidenceError, SourceLocation, SourceUse, SourceUseRole,
    SourceUses, TagResolver,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn tag(id: u128, name: &str, children: Vec<TestValue>) -> TestRecord {
    TestRecord::new(id, "Tag", format!("Tag.{name}"), "synthetic/tags.xml")
        .value("tagName", TestValue::text(name))
        .value("children", TestValue::Array(children))
}

fn bytes() -> Vec<u8> {
    let reference = || TestType::ReferenceTo("Tag".to_owned());
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", reference()),
        )
        .structure(TestStruct::new("TagDatabase").array("tags", reference()))
        .record(tag(
            1,
            "Root",
            vec![TestValue::Null, TestValue::Reference(2)],
        ))
        .record(tag(2, "Child", Vec::new()))
        .record(tag(3, "Loose", Vec::new()))
        .record(
            TestRecord::new(4, "TagDatabase", "TagDatabase.Roots", "synthetic/roots.xml").value(
                "tags",
                TestValue::Array(vec![TestValue::Null, TestValue::Reference(1)]),
            ),
        )
        .record(tag(2, "Shadow", Vec::new()))
        .build()
}

#[test]
fn tag_sources_follow_the_selected_occurrences_of_the_exact_input() -> TestResult {
    let folder = tempfile::tempdir()?;
    let file = folder.path().join("synthetic.dcb");
    let bytes = bytes();
    fs::write(&file, &bytes)?;
    let input = Arc::new(DataForgeEntry::read_file(&file)?);
    let forge = input.open()?;
    let same = input.open()?;
    let other = DataForgeEntry::read_file(&file)?;
    let foreign = other.open()?;
    let uses = SourceUses::new();
    assert!(
        matches!(
            ObservedTagTree::build(Arc::clone(&input), &foreign, &|| false),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "another read of identical bytes cannot supply the positions"
    );
    assert!(
        ObservedTagTree::build(Arc::clone(&input), &forge, &|| true)?.is_none(),
        "cancellation returns no tree or proof"
    );
    let tags =
        ObservedTagTree::build(Arc::clone(&input), &forge, &|| false)?.ok_or("observed tree")?;
    assert!(
        matches!(
            tags.bind_paths(&foreign, &uses),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "a foreign DataForge cannot bind the cached coordinates"
    );
    let child = forge.record(1).ok_or("child record")?.guid();
    assert_eq!(
        tags.tag_path(child),
        Some("Root.Child"),
        "the resolver uses the observed physical path"
    );
    let paths = tags.bind_paths(&same, &uses)?;
    let mut inputs = Vec::new();
    assert!(paths.append(child, &mut inputs)?, "child path is complete");
    assert_tag_occurrences(&inputs, &bytes);
    let origin = uses.origin(&inputs, true);
    assert_eq!(origin.contains_encrypted_source(), None, "standalone input");

    inputs.clear();
    let loose = forge.record(2).ok_or("unlisted record")?.guid();
    assert!(paths.append(loose, &mut inputs)?, "unlisted root name");
    assert_eq!(inputs.len(), 2, "there is no authored incoming root edge");
    Ok(())
}

fn assert_tag_occurrences(inputs: &[SourceUse], bytes: &[u8]) {
    let edges: Vec<_> = inputs
        .iter()
        .filter_map(|used| match used.description().location() {
            SourceLocation::Subresource {
                namespace,
                field,
                indices,
            } if namespace == "dataforgeTagRelations" && used.role() == SourceUseRole::Selector => {
                Some((field.as_str(), indices.as_slice()))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        edges,
        [
            ("children", [0_u64, 1].as_slice()),
            ("tags", [3_u64, 1].as_slice())
        ],
        "original pre-filter slots and physical owners"
    );
    let names: Vec<_> = inputs
        .iter()
        .filter_map(|used| match used.description().location() {
            SourceLocation::DataForge {
                record_index,
                field,
            } if field == "tagName" && used.role() == SourceUseRole::Conversion => {
                Some(*record_index)
            }
            _ => None,
        })
        .collect();
    assert_eq!(names, [1, 0], "first physical body and selected parent");
    let ranges: Vec<_> = inputs
        .iter()
        .filter_map(|used| match used.description().location() {
            SourceLocation::Subresource {
                namespace,
                field,
                indices,
            } if namespace == "dataforgeDecodedBytes" && field == "text" => {
                let &[start, end] = indices.as_slice() else {
                    return None;
                };
                bytes.get(usize::try_from(start).ok()?..usize::try_from(end).ok()?)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        ranges,
        [b"Child".as_slice(), b"Root".as_slice()],
        "original name bytes"
    );
}
