// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForgeEntry, Guid, RecordTree};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

use super::{CatalogOrigins, Evidence, OriginalCatalogInput, ShipDependencies, TreeBuilder};
use crate::playability::Playability;
use crate::playability_rules::ship_decision;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SHARED: u128 = 0xa1;
const INDEPENDENT: u128 = 0xa2;

#[test]
fn playability_groups_preserve_physical_dependency_evidence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("playability.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let origins = CatalogOrigins::for_archive_records(&archive, &entry)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    let mut builder = TreeBuilder::new();
    let owners = forge
        .records()
        .filter(|record| {
            record
                .type_name()
                .is_ok_and(|name| name == "EntityClassDefinition")
        })
        .map(|record| builder.build(&record))
        .collect::<Result<Vec<_>, _>>()?;
    let [alpha, bravo, charlie] = owners.as_slice() else {
        return Err("three physical owner trees".into());
    };
    verify_groups(&input, [alpha, bravo, charlie])
}

fn verify_groups<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    [alpha, bravo, charlie]: [&RecordTree<'source>; 3],
) -> TestResult {
    let shared: Guid = "00000000-0000-0000-0000-0000000000a1".parse()?;
    let independent: Guid = "00000000-0000-0000-0000-0000000000a2".parse()?;
    let mut cache = ShipDependencies::new(input);
    let first = cache.prepare([alpha], None)?;
    assert_eq!(
        first.sources.len(),
        2,
        "both physical aliases are inspected"
    );
    let first_origin = cache.evidence().provenance(input, &[shared])?;
    assert!(
        !first_origin.source_set_complete(),
        "the unreadable alias makes the shared lookup incomplete"
    );
    let second = cache.prepare([bravo], None)?;
    assert_eq!(
        second.sources.len(),
        1,
        "only the new independent dependency is read"
    );
    let third = cache.prepare([charlie], None)?;
    assert_eq!(
        third.sources.len(),
        0,
        "the previously read independent target is reused"
    );

    let (whole, _) = Evidence::read_for_ships([alpha, bravo, charlie], input, None)?;
    let batched = cache.evidence();
    for targets in [vec![shared], vec![shared, independent], vec![independent]] {
        assert_eq!(
            batched.provenance(input, &targets)?,
            whole.provenance(input, &targets)?,
            "both preparations bind exactly the original targets {targets:?}",
        );
    }
    assert_eq!(
        batched.provenance(input, &[shared])?,
        first_origin,
        "later groups cannot change earlier proof",
    );
    assert_eq!(
        ship_decision(alpha, None, batched),
        (None, true),
        "the unreadable physical alias takes precedence over the good one",
    );
    assert_eq!(
        ship_decision(charlie, None, batched),
        (Some(Playability::Mission), false),
        "the independent valid tag cannot inherit the failure",
    );
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .record(tag(SHARED, "main"))
        .record(tag(SHARED, "bad_alias").with_instance_index(u16::MAX))
        .record(tag(INDEPENDENT, "independent"))
        .record(owner(0xb1, "alpha", &[SHARED]))
        .record(owner(0xb2, "bravo", &[SHARED, INDEPENDENT]))
        .record(owner(0xb3, "charlie", &[INDEPENDENT]))
        .build()
}

fn tag(guid: u128, stem: &str) -> TestRecord {
    TestRecord::new(guid, "Tag", stem, format!("synthetic/{stem}.xml"))
        .value("tagName", TestValue::text("Unmanned"))
}

fn owner(guid: u128, stem: &str, tags: &[u128]) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        stem,
        format!("synthetic/{stem}.xml"),
    )
    .value(
        "tags",
        TestValue::Array(tags.iter().copied().map(TestValue::Reference).collect()),
    )
}
