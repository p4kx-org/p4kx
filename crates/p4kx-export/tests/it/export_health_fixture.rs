// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::CatalogOptions;
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

const MISSING: u128 = 0x9900_0000_0000_4000_8000_0000_0000_0001;

pub(super) fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
}

pub(super) fn archive(fault: bool) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database(fault)))
        .write_to(&path)?;
    Ok((root, Archive::open(&path)?))
}

fn database(fault: bool) -> Vec<u8> {
    schema()
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.Healthy",
                "libs/foundry/records/scitemmanufacturer/healthy.xml",
            )
            .value("Code", TestValue::text("OK")),
        )
        .record(inspected(fault))
        .record(dangling_item())
        .record(TestRecord::new(
            3,
            "EntityClassDefinition",
            "EntityClassDefinition.DebugNoCrafter",
            "libs/foundry/records/entities/props/debug_no_crafter.xml",
        ))
        .build()
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Leaf"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("Component".to_owned()),
        ))
        .structure(TestStruct::new("Component"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("Component")
                .field(
                    "AttachDef",
                    TestType::StrongPointer("SItemDefinition".to_owned()),
                ),
        )
        .structure(TestStruct::new("SItemDefinition").field(
            "Manufacturer",
            TestType::ReferenceTo("SCItemManufacturer".to_owned()),
        ))
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field("Localization", TestType::StrongPointer("Leaf".to_owned())),
        )
}

fn inspected(fault: bool) -> TestRecord {
    TestRecord::new(
        2,
        "SCItemManufacturer",
        "SCItemManufacturer.Inspected",
        "libs/foundry/records/scitemmanufacturer/inspected.xml",
    )
    .value(
        "Code",
        if fault {
            TestValue::Raw((u32::MAX - 1).to_le_bytes().to_vec())
        } else {
            TestValue::text("")
        },
    )
}

fn dangling_item() -> TestRecord {
    TestRecord::new(
        4,
        "EntityClassDefinition",
        "EntityClassDefinition.DanglingItem",
        "libs/foundry/records/entities/scitem/dangling.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("SAttachableComponentParams").value(
                "AttachDef",
                TestValue::Struct(
                    TestInstance::new("SItemDefinition")
                        .value("Manufacturer", TestValue::Reference(MISSING)),
                ),
            ),
        )]),
    )
}
