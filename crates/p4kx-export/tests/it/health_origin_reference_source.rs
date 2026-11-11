// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_fixture::{self as fixture, TestResult};
use p4kx_extraction::{DataForge, Field, RecordTree, Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const OWNER: u128 = 0x5721;
pub(crate) const TARGET: u128 = 0x5720;
pub(crate) const MISSING: u128 = 0x572f;
pub(crate) const OWNER_PATH: &str = "libs/foundry/records/scitemmanufacturer/probe";

pub(crate) fn database() -> Vec<u8> {
    let mut words = (u32::MAX - 1).to_le_bytes().to_vec();
    words.extend(7_u32.to_le_bytes());
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field(
                    "Resolved",
                    TestType::ReferenceTo("SCItemManufacturer".to_owned()),
                )
                .field(
                    "Dangling",
                    TestType::ReferenceTo("SCItemManufacturer".to_owned()),
                )
                .field("Owned", TestType::StrongPointer("HealthLeaf".to_owned()))
                .field("Again", TestType::StrongPointer("HealthLeaf".to_owned()))
                .field("Weak", TestType::WeakPointer("HealthLeaf".to_owned())),
        )
        .structure(TestStruct::new("HealthLeaf").field("Probe", TestType::UInt32))
        .record(
            TestRecord::new(
                TARGET,
                "SCItemManufacturer",
                "SCItemManufacturer.Target",
                "libs/foundry/records/scitemmanufacturer/target.xml",
            )
            .value("Code", TestValue::text("TARGET")),
        )
        .record(
            TestRecord::new(
                OWNER,
                "SCItemManufacturer",
                "SCItemManufacturer.Probe",
                format!("{OWNER_PATH}.xml"),
            )
            .value("Code", TestValue::text("OWNER"))
            .value("Resolved", TestValue::Reference(TARGET))
            .value("Dangling", TestValue::Reference(MISSING))
            .value(
                "Owned",
                TestValue::Struct(TestInstance::new("HealthLeaf").named("health-owned")),
            )
            .value("Again", TestValue::pointer("health-owned"))
            .value("Weak", TestValue::Raw(words)),
        )
        .build()
}

pub(crate) fn original(bytes: &[u8]) -> TestResult<(u32, u32)> {
    let forge = DataForge::open(bytes)?;
    let owner = forge.record(1).ok_or("original owning Record")?;
    assert_eq!(
        owner.guid().to_string(),
        fixture::guid(OWNER),
        "the original owner is independent of every target"
    );
    assert_eq!(
        owner.record_path()?,
        OWNER_PATH,
        "the original owner path is readable"
    );
    let tree = owner.tree()?;
    original_positions(&tree)?;
    let Some(Raw::Reference(reference)) = tree.root().field("Resolved") else {
        return Err("original resolved reference".into());
    };
    assert_eq!(
        reference.guid().to_string(),
        fixture::guid(TARGET),
        "the actual resolved reference target is real"
    );
    assert_eq!(
        reference.record_index(),
        0,
        "the target header is a different physical owner"
    );
    assert!(
        matches!(tree.root().field("Dangling"), Some(Raw::Error(ValueError::DanglingReference { guid })) if guid.to_string() == fixture::guid(MISSING)),
        "the stored dangling GUID is retained by the original decoder"
    );
    assert!(
        matches!(
            tree.root().field("Again"),
            Some(Raw::Error(ValueError::OwnedTwice { .. }))
        ),
        "the actual strong pointer owns the same original instance twice"
    );
    let Some(Raw::Error(ValueError::InvalidPointer {
        struct_index,
        instance_index,
    })) = tree.root().field("Weak")
    else {
        return Err("original invalid pointer words".into());
    };
    assert_eq!(
        (*struct_index, *instance_index),
        (u32::MAX - 1, 7),
        "the actual decoder retains the exact stored words"
    );
    Ok((*struct_index, *instance_index))
}

fn original_positions(tree: &RecordTree<'_>) -> TestResult {
    let names: Vec<_> = tree.root().fields().iter().map(Field::name).collect();
    assert_eq!(
        names,
        ["Code", "Resolved", "Dangling", "Owned", "Again", "Weak"],
        "the exact original root field order fixes the Reference positions"
    );
    let Some(Raw::Struct(owned)) = tree.root().field("Owned") else {
        return Err("original owned subtree before the later pointer".into());
    };
    let nested: Vec<_> = owned.fields().iter().map(Field::name).collect();
    assert_eq!(
        nested,
        ["Probe"],
        "the original owned field contributes one DFS value before Again and Weak"
    );
    Ok(())
}
