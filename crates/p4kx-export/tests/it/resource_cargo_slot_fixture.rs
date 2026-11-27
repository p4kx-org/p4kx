// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::commodity_v3_fixture::TestResult;

pub(crate) const RESOURCE: u128 = 0x8c01;
pub(crate) const FIRST: u128 = 0x8c02;
pub(crate) const SECOND: u128 = 0x8c03;
const SLOT_TYPE: &str = "SResourceTypeDefaultCargoContainers";

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ResourceType").field(
            "defaultCargoContainers",
            TestType::StrongPointer(SLOT_TYPE.to_owned()),
        ))
        .structure(
            TestStruct::new(SLOT_TYPE)
                .field(
                    "oneSCU",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                )
                .field(
                    "twoSCU",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
        .structure(TestStruct::new("FixtureComponent"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("FixtureComponent".to_owned()),
        ))
        .record(
            TestRecord::new(
                RESOURCE,
                "ResourceType",
                "ResourceType.DuplicateSlots",
                "libs/foundry/records/resourcetypedatabase/duplicate_slots.xml",
            )
            .value(
                "defaultCargoContainers",
                TestValue::Struct(
                    TestInstance::new(SLOT_TYPE)
                        .value("oneSCU", TestValue::Reference(FIRST))
                        .value("twoSCU", TestValue::Reference(SECOND)),
                ),
            ),
        )
        .record(TestRecord::new(
            FIRST,
            "EntityClassDefinition",
            "EntityClassDefinition.SlotFirst",
            "libs/foundry/records/entities/fixture/slot_first.xml",
        ))
        .record(TestRecord::new(
            SECOND,
            "EntityClassDefinition",
            "EntityClassDefinition.SlotSecond",
            "libs/foundry/records/entities/fixture/slot_second.xml",
        ))
}

fn renamed_database() -> TestResult<Vec<u8>> {
    const ORIGINAL: &[u8] = b"twoSCU\0";
    const REPLACEMENT: &[u8] = b"oneSCU\0";
    let mut data = database().build();
    let positions: Vec<_> = data
        .windows(ORIGINAL.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == ORIGINAL).then_some(position))
        .collect();
    let [position] = positions.as_slice() else {
        return Err("one unique synthetic cargo declaration name".into());
    };
    assert_eq!(
        ORIGINAL.len(),
        REPLACEMENT.len(),
        "renaming preserves all offsets and both original target values"
    );
    data.get_mut(*position..*position + ORIGINAL.len())
        .ok_or("original field-name bytes")?
        .copy_from_slice(REPLACEMENT);
    Ok(data)
}

pub(crate) fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("duplicate-cargo-fields.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", renamed_database()?))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}
