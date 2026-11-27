// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityNode, LoadoutResolver};
use p4kx_dataforge::{DataForge, RecordTree};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestGroup, TestLoadout, TestLoadoutEntry, TestRecord,
    TestRequirement, TestValue,
};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const ASSORTMENT: u128 = 0x100;
const TAG: u128 = 0x500;

#[derive(Clone, Copy)]
pub(crate) enum Site {
    Assortment,
    Group,
}

pub(crate) fn nested(depth: usize, terminal: TestRequirement) -> TestRequirement {
    (0..depth).fold(terminal, |part, _| TestRequirement::Not(Box::new(part)))
}

pub(crate) fn tagged() -> TestRequirement {
    TestRequirement::Tags {
        required: vec![TAG],
        banned: Vec::new(),
    }
}

pub(crate) fn bytes(site: Site, requirement: Option<TestRequirement>) -> Vec<u8> {
    database(record(site, requirement))
}

pub(crate) fn raw(value: TestValue) -> Vec<u8> {
    database(record(Site::Assortment, None).value("Requirement", value))
}

pub(crate) fn missing() -> TestResult<Vec<u8>> {
    let mut bytes = bytes(Site::Assortment, None);
    let name = b"Requirement\0";
    let positions: Vec<_> = bytes
        .windows(name.len())
        .enumerate()
        .filter_map(|(at, part)| (part == name).then_some(at))
        .collect();
    assert_eq!(positions.len(), 1, "one genuine interned declaration name");
    let at = positions
        .first()
        .copied()
        .ok_or("original Requirement declaration name")?;
    *bytes.get_mut(at).ok_or("original declaration name byte")? = b'r';
    Ok(bytes)
}

fn record(site: Site, requirement: Option<TestRequirement>) -> TestRecord {
    let manual =
        |port: &str, class: &str| TestLoadout::Manual(vec![TestLoadoutEntry::class(port, class)]);
    let body = TestGroup::new("Body", vec![manual("Body_ItemPort", "Body_Red")]);
    let (body, root) = match site {
        Site::Assortment => (body, requirement),
        Site::Group => (
            match requirement {
                Some(requirement) => body.requiring(requirement),
                None => body,
            },
            None,
        ),
    };
    let groups = [
        body,
        TestGroup::new("Legs", vec![manual("Legs_ItemPort", "Body_Blue")]),
    ];
    p4kx_testkit::assortment_record_requiring(ASSORTMENT, "complete", root.as_ref(), &groups)
}

fn database(record: TestRecord) -> Vec<u8> {
    let builder = [
        TestEntity::new(1, "Beast", "entities/beast.xml")
            .loadout(TestLoadout::RandomGenerated(vec![ASSORTMENT])),
        TestEntity::new(2, "Body_Red", "entities/body_red.xml"),
        TestEntity::new(3, "Body_Blue", "entities/body_blue.xml"),
    ]
    .iter()
    .fold(
        p4kx_testkit::entity_schema(DataForgeBuilder::new()),
        |source, entity| source.record(entity.record()),
    );
    builder
        .record(record)
        .record(TestRecord::new(
            TAG,
            "Tag",
            "Tag.Required",
            "tags/required.xml",
        ))
        .build()
}

pub(crate) fn run(
    bytes: &[u8],
    original: impl FnOnce(&RecordTree<'_>) -> TestResult,
    check: impl FnOnce(&EntityNode),
) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let records: Vec<_> = forge
        .records()
        .filter(|record| record.type_name().ok() == Some("SLoadoutAssortment"))
        .collect();
    assert_eq!(records.len(), 1, "the actual original assortment is unique");
    let record = records.first().ok_or("the original assortment")?;
    let tree = record.tree()?;
    assert_eq!(
        tree.errors().count(),
        0,
        "the complete original canonical source has no read/value errors"
    );
    original(&tree)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let beast = resolver.entities().find("Beast")?;
    let node = resolver.resolve(&beast)?;
    check(&node);
    Ok(())
}
