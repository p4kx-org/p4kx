// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Weak pointers, Records that share a file, the order of a limited index and cancelling the build.

use p4kx_dataforge::{DataForge, MAX_INDEXED_REFERENCES, ReferenceIndex};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ONE: u128 = 1;
const TWO: u128 = 2;

fn in_both_versions(
    builder: &DataForgeBuilder,
    check: impl Fn(&DataForge<'_>) -> TestResult,
) -> TestResult {
    for version in [6, 8] {
        let bytes = builder.clone().version(version).build();
        let forge = DataForge::open(&bytes)?;
        check(&forge).map_err(|error| format!("version {version}: {error}"))?;
    }
    Ok(())
}

fn scenes() -> DataForgeBuilder {
    let node = || TestType::WeakPointer("Node".to_owned());
    let scene = || TestType::WeakPointer("Scene".to_owned());
    DataForgeBuilder::new()
        .structure(TestStruct::new("Node").field("name", TestType::String))
        .structure(
            TestStruct::new("Scene")
                .array("nodes", TestType::StrongPointer("Node".to_owned()))
                .field("other", scene())
                .field("deep", node())
                .field("myself", scene())
                .field("inside", node()),
        )
        .instance("mine", TestInstance::new("Node"))
        .instance("sub", TestInstance::new("Node"))
        .record(
            TestRecord::new(ONE, "Scene", "Scene.One", "scenes/one.xml")
                .value("nodes", TestValue::Array(vec![TestValue::pointer("mine")]))
                .value("other", TestValue::pointer("Scene.Two"))
                .value("deep", TestValue::pointer("sub"))
                .value("myself", TestValue::pointer("Scene.One"))
                .value("inside", TestValue::pointer("mine")),
        )
        .record(
            TestRecord::new(TWO, "Scene", "Scene.Two", "scenes/two.xml")
                .value("nodes", TestValue::Array(vec![TestValue::pointer("sub")]))
                .value("other", TestValue::pointer("Scene.One")),
        )
}

fn record(forge: &DataForge<'_>, name: &str) -> Result<u32, String> {
    forge
        .records()
        .find(|record| record.name().is_ok_and(|found| found == name))
        .map(|record| record.index())
        .ok_or_else(|| format!("no Record {name}"))
}

fn locations(index: &ReferenceIndex, source: u32) -> Vec<String> {
    index.outgoing(source).map(|link| link.location()).collect()
}

#[test]
fn a_weak_pointer_to_another_record_is_a_reference() -> TestResult {
    in_both_versions(&scenes(), |forge| {
        let index = ReferenceIndex::build(forge);
        let (one, two) = (record(forge, "Scene.One")?, record(forge, "Scene.Two")?);
        let from_one: Vec<_> = index
            .outgoing(one)
            .map(|link| (link.location(), link.target()))
            .collect();
        assert_eq!(
            from_one,
            [("other".to_owned(), Some(two))],
            "only the pointer to the other Record"
        );
        let to_one: Vec<_> = index.incoming(one).map(|link| link.source()).collect();
        assert_eq!(to_one, [two], "the pointer of the other Record back");
        Ok(())
    })
}

#[test]
fn weak_pointers_into_the_own_record_are_not_indexed() -> TestResult {
    in_both_versions(&scenes(), |forge| {
        let index = ReferenceIndex::build(forge);
        let one = record(forge, "Scene.One")?;
        let own: Vec<_> = locations(&index, one)
            .into_iter()
            .filter(|at| at == "myself" || at == "inside")
            .collect();
        assert!(own.is_empty(), "{own:?}");
        assert_eq!(index.len(), 2, "one pointer each way and nothing else");
        Ok(())
    })
}

#[test]
fn a_weak_pointer_into_a_non_root_instance_of_another_record_is_skipped() -> TestResult {
    in_both_versions(&scenes(), |forge| {
        let index = ReferenceIndex::build(forge);
        let one = record(forge, "Scene.One")?;
        assert!(
            !locations(&index, one).contains(&"deep".to_owned()),
            "the pointer to an instance owned by another Record is no Reference"
        );
        assert_eq!(index.unresolved_count(), 0, "and it is no unresolved one");
        Ok(())
    })
}

fn shared_file() -> DataForgeBuilder {
    let link = || TestType::ReferenceTo("Node".to_owned());
    let node = |guid, name, file, target: Option<u128>| {
        let record = TestRecord::new(guid, "Node", name, file);
        match target {
            Some(target) => record.value("next", TestValue::Reference(target)),
            None => record,
        }
    };
    DataForgeBuilder::new()
        .structure(TestStruct::new("Node").field("next", link()))
        .record(node(1, "Node.A", "nodes/shared.xml", Some(2)))
        .record(node(2, "Node.B", "nodes/shared.xml", Some(3)))
        .record(node(3, "Node.C", "nodes/other.xml", Some(2)))
}

#[test]
fn a_record_that_shares_its_file_with_an_earlier_one_is_indexed_both_ways() -> TestResult {
    in_both_versions(&shared_file(), |forge| {
        let index = ReferenceIndex::build(forge);
        let (a, b, c) = (
            record(forge, "Node.A")?,
            record(forge, "Node.B")?,
            record(forge, "Node.C")?,
        );
        assert_eq!(locations(&index, b), ["next"], "B holds a Reference");
        assert_eq!(index.outgoing(b).next().and_then(|l| l.target()), Some(c));
        assert_eq!(index.referrers(b), [a, c], "A and C refer to B");
        assert_eq!(index.referrers(c), [b], "B refers to C");
        Ok(())
    })
}

fn chain(records: u128) -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .structure(TestStruct::new("Link").field("next", TestType::Reference));
    (1..=records).fold(builder, |builder, guid| {
        let name = format!("Link.{guid}");
        let path = format!("links/{guid}.xml");
        let record = TestRecord::new(guid, "Link", name, path);
        builder.record(if guid < records {
            record.value("next", TestValue::Reference(guid + 1))
        } else {
            record
        })
    })
}

fn pairs(index: &ReferenceIndex, count: u32) -> Vec<(u32, Option<u32>, String)> {
    (0..count)
        .flat_map(|record| {
            index
                .outgoing(record)
                .map(|link| (link.source(), link.target(), link.location()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn limited(forge: &DataForge<'_>, limit: usize) -> Result<ReferenceIndex, &'static str> {
    ReferenceIndex::build_with(forge, limit, &|| false).ok_or("an index without a stop")
}

#[test]
fn a_limited_index_keeps_the_first_references_in_record_order() -> TestResult {
    let bytes = chain(1_300).build();
    let forge = DataForge::open(&bytes)?;
    let count = u32::try_from(forge.record_count())?;
    let full = ReferenceIndex::build(&forge);
    let all = pairs(&full, count);
    assert_eq!(all.len(), 1_299, "every Record but the last refers on");
    for limit in [0, 1, 511, 512, 513, 700, 1_024, 1_298] {
        let capped = limited(&forge, limit)?;
        assert!(capped.is_truncated(), "limit {limit}");
        assert_eq!(
            pairs(&capped, count),
            all[..limit],
            "limit {limit}: the first References of the Records in order"
        );
    }
    assert!(
        !limited(&forge, 1_299)?.is_truncated(),
        "a limit equal to the number is no truncation"
    );
    Ok(())
}

#[test]
fn a_limited_index_does_not_depend_on_the_number_of_threads() -> TestResult {
    let bytes = chain(2_600).build();
    let forge = DataForge::open(&bytes)?;
    let count = u32::try_from(forge.record_count())?;
    let single = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    let wide = rayon::ThreadPoolBuilder::new().num_threads(8).build()?;
    let narrow = single.install(|| limited(&forge, 2_200))?;
    let broad = wide.install(|| limited(&forge, 2_200))?;
    assert_eq!(pairs(&narrow, count), pairs(&broad, count), "same cut");
    assert_eq!(narrow.len(), 2_200, "cut at the limit");
    Ok(())
}

#[test]
fn a_cancelled_build_gives_no_index() -> TestResult {
    let bytes = chain(2_600).build();
    let forge = DataForge::open(&bytes)?;
    assert!(
        ReferenceIndex::build_with(&forge, MAX_INDEXED_REFERENCES, &|| true).is_none(),
        "stopped before the first Record"
    );
    let built = ReferenceIndex::build_with(&forge, MAX_INDEXED_REFERENCES, &|| false);
    assert_eq!(built.map(|index| index.len()), Some(2_599), "not stopped");
    Ok(())
}

#[test]
fn aliases_keep_the_actual_physical_weak_pointer_target() -> TestResult {
    let aliases = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Alias").field("link", TestType::WeakPointer("Alias".to_owned())),
        )
        .record(
            TestRecord::new(ONE, "Alias", "Alias.First", "aliases/first.xml")
                .value("link", TestValue::pointer("Alias.Second")),
        )
        .record(TestRecord::new(
            ONE,
            "Alias",
            "Alias.Second",
            "aliases/second.xml",
        ))
        .record(
            TestRecord::new(TWO, "Alias", "Alias.Caller", "aliases/caller.xml")
                .value("link", TestValue::pointer("Alias.Second")),
        );
    in_both_versions(&aliases, |forge| {
        let first = record(forge, "Alias.First")?;
        let second = record(forge, "Alias.Second")?;
        let caller = record(forge, "Alias.Caller")?;
        let target = forge.record(second).ok_or("the second physical alias")?;
        assert_eq!(
            forge
                .record_by_guid(target.guid())
                .map(|record| record.index()),
            Some(first)
        );
        for source in [first, caller] {
            let original = forge.record(source).ok_or("the physical caller")?.tree()?;
            let Some(p4kx_dataforge::Value::Reference(reference)) = original.field("link") else {
                return Err("the original decoder admits the other physical instance".into());
            };
            assert_eq!(
                reference.record_index(),
                second,
                "the source selects the second alias"
            );
            assert!(
                reference.location().is_none(),
                "the target is a Record root"
            );
        }
        let index = ReferenceIndex::build(forge);
        assert_eq!(index.referrers(second), [first, caller]);
        assert!(index.incoming(first).next().is_none());
        for source in [first, caller] {
            let links: Vec<_> = index
                .outgoing(source)
                .map(|link| (link.location(), link.target()))
                .collect();
            assert_eq!(links, [("link".to_owned(), Some(second))]);
        }
        Ok(())
    })
}
