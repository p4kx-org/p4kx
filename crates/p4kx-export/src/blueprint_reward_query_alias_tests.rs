// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, ValueKind};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{BlueprintRewardSelection, CatalogRecordSelection, TestResult, fixture, query};

#[test]
fn unknown_pool_alias_headers_remain_distinct_from_proven_foreign_headers() -> TestResult {
    for (path, expected) in [
        (
            "libs/foundry/records/crafting/blueprintrewards/unknown.xml",
            vec![8],
        ),
        ("outside/unknown.xml", vec![]),
    ] {
        let database = fixture::database(false)
            .structure(TestStruct::new("UnknownPool"))
            .record(TestRecord::new(
                fixture::POOL,
                "UnknownPool",
                "UnknownPool.Alias",
                path,
            ));
        let mut bytes = database.build();
        corrupt_name(&mut bytes, b"UnknownPool\0")?;
        let forge = DataForge::open(&bytes)?;
        assert!(
            forge.record(8).ok_or("unknown alias")?.type_name().is_err(),
            "header error is lazy and reachable"
        );
        let selection = BlueprintRewardSelection::SourceOwners(
            CatalogRecordSelection::RecordIndex(fixture::SOURCE),
        );
        let (found, reads) = query(bytes, selection)?;
        assert_eq!(
            found.rows.len(),
            4,
            "a readable alias keeps canonical nominal membership"
        );
        assert_eq!(
            found.pools.unreadable_headers, expected,
            "only possible pool candidates affect coverage"
        );
        assert_eq!(
            reads,
            [(3, true), (0, true), (1, true)],
            "unclassifiable and foreign bodies are not guessed"
        );
    }
    Ok(())
}

fn corrupt_name(bytes: &mut [u8], needle: &[u8]) -> TestResult {
    let position = {
        let mut found = bytes
            .windows(needle.len())
            .enumerate()
            .filter(|(_, value)| *value == needle);
        let (position, _) = found.next().ok_or("exact original Struct name token")?;
        assert!(found.next().is_none(), "Struct name token is unique");
        position
    };
    *bytes.get_mut(position).ok_or("Struct name byte")? = 0xff;
    Ok(())
}

#[test]
fn failed_actual_target_can_keep_its_original_link_through_a_materialized_guid_alias() -> TestResult
{
    let database = failed_alias_database();
    let mut bytes = database.build();
    share_struct_name(&mut bytes, 0, 1)?;
    let forge = DataForge::open(&bytes)?;
    assert!(
        forge.record(0).ok_or("readable pool")?.tree().is_ok(),
        "admissible alias body is readable"
    );
    assert!(
        forge.record(1).ok_or("failed pool")?.tree().is_err(),
        "selected physical target really fails"
    );
    let selection = BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(1));
    let (found, _) = query(bytes, selection)?;
    let [row] = found.rows.as_slice() else {
        return Err("one actual failed-target link".into());
    };
    assert_eq!(
        row.2, 1,
        "the reference is not rewritten to the readable sibling"
    );
    assert_eq!(
        found.pools.unreadable_bodies,
        [1],
        "target definition coverage stays honestly incomplete"
    );
    Ok(())
}

fn failed_alias_database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("BlueprintPoolRecord"))
        .structure(
            TestStruct::new("BrokenPool")
                .parent("BlueprintPoolRecord")
                .field("bad", TestType::UInt64)
                .declared_size(1),
        )
        .structure(TestStruct::new("Owner").field(
            "pool",
            TestType::WeakPointer("BlueprintPoolRecord".to_owned()),
        ))
        .record(TestRecord::new(
            1,
            "BlueprintPoolRecord",
            "BlueprintPoolRecord.Primary",
            "libs/foundry/records/crafting/blueprintrewards/primary.xml",
        ))
        .record(TestRecord::new(
            1,
            "BrokenPool",
            "BrokenPool.Alias",
            "libs/foundry/records/crafting/blueprintrewards/alias.xml",
        ))
        .record(
            TestRecord::new(2, "Owner", "Owner.Source", "outside/source.xml")
                .value("pool", TestValue::pointer("BrokenPool.Alias")),
        )
}

fn share_struct_name(bytes: &mut [u8], from_record: u32, to_record: u32) -> TestResult {
    let (from, to) = {
        let forge = DataForge::open(bytes)?;
        let from = forge
            .record(from_record)
            .ok_or("readable original Record")?
            .struct_type()?
            .index();
        let to = forge
            .record(to_record)
            .ok_or("target original Record")?
            .struct_type()?
            .index();
        (from, to)
    };
    let header_bytes = (9 + ValueKind::ALL.len() + 2) * size_of::<u32>();
    let struct_bytes = 16;
    let source = header_bytes + usize::try_from(from)? * struct_bytes;
    let target = header_bytes + usize::try_from(to)? * struct_bytes;
    let original: [u8; 4] = bytes
        .get(source..source + 4)
        .ok_or("original Struct name offset")?
        .try_into()?;
    bytes
        .get_mut(target..target + 4)
        .ok_or("target Struct name offset")?
        .copy_from_slice(&original);
    Ok(())
}
