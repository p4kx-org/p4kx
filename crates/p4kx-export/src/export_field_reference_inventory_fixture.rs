// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid, RecordTree, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::FieldInspection;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(super) const NONFINITE_BITS: u32 = 0x7fc0_0017;

#[derive(Clone, Copy)]
pub(super) enum Case {
    Count,
    Metadata,
}

impl Case {
    pub(super) const fn references(self) -> usize {
        match self {
            Self::Count => 4097,
            Self::Metadata => 2,
        }
    }

    fn target_file(self) -> String {
        match self {
            Self::Count => "manufacturers/target.xml".to_owned(),
            Self::Metadata => format!("manufacturers/{}.xml", "m".repeat(1024 * 1024 + 1)),
        }
    }
}

fn bytes(case: Case) -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Source")
                .array(
                    "links",
                    TestType::ReferenceTo("SCItemManufacturer".to_owned()),
                )
                .field(
                    "lateDangling",
                    TestType::ReferenceTo("SCItemManufacturer".to_owned()),
                )
                .field("lateBad", TestType::Bool)
                .field("lateNumber", TestType::Single),
        )
        .structure(TestStruct::new("SCItemManufacturer"))
        .record(
            TestRecord::new(0x6811, "Source", "Source.One", "source.xml")
                .value(
                    "links",
                    TestValue::Array(vec![TestValue::Reference(0x6812); case.references()]),
                )
                .value("lateDangling", TestValue::Reference(0x6813))
                .value("lateBad", TestValue::Raw(vec![2]))
                .value(
                    "lateNumber",
                    TestValue::Single(f32::from_bits(NONFINITE_BITS)),
                ),
        )
        .record(TestRecord::new(
            0x6812,
            "SCItemManufacturer",
            "SCItemManufacturer.Target",
            case.target_file(),
        ))
        .build()
}

pub(super) fn inspect(
    case: Case,
    check: impl FnOnce(FieldInspection, usize) -> TestResult,
) -> TestResult {
    let bytes = bytes(case);
    let forge = DataForge::open(&bytes)?;
    let owner = forge.record(0).ok_or("actual original owner")?;
    let target = forge.record(1).ok_or("actual original target")?;
    let tree = owner.tree()?;
    assert!(
        tree.is_from(&owner),
        "the actual canonical tree belongs to this Record/Forge"
    );
    assert_eq!(
        target.type_name()?,
        "SCItemManufacturer",
        "the actual target is a catalog Reference family"
    );
    original(&forge, &tree, target.guid(), target.record_path()?, case)?;
    let ledger = super::super::inspect(&tree, &owner, None, ExportSourceRole::Dependency, None)?;
    let mut inspections = ledger.field_inspections.into_iter();
    let inspection = inspections
        .next()
        .ok_or("one actual canonical inspection")?;
    assert!(
        inspections.next().is_none(),
        "the one real read produces exactly one inspection"
    );
    check(inspection, bytes.len())
}

fn original(
    forge: &DataForge<'_>,
    tree: &RecordTree<'_>,
    target: Guid,
    path: &str,
    case: Case,
) -> TestResult {
    let Some(Value::Array(links)) = tree.field("links") else {
        return Err("actual original reference array".into());
    };
    assert_eq!(
        links.len(),
        case.references(),
        "every physical source Reference slot is materialized before inspection"
    );
    for value in links {
        let Value::Reference(reference) = value else {
            return Err("actual original resolved Reference".into());
        };
        assert_eq!(
            reference.guid(),
            target,
            "actual canonical target comes from the original stored Reference"
        );
        assert_eq!(
            reference.record_index(),
            1,
            "original target physical index"
        );
        assert_eq!(
            reference.record_path(),
            path,
            "the original target path is retained before any Inspector budget"
        );
    }
    if matches!(case, Case::Metadata) {
        assert!(
            path.len() > 1024 * 1024,
            "one actual retained target path independently exceeds the old metadata cap"
        );
    }
    let Some(Value::Error(ValueError::DanglingReference { guid })) = tree.field("lateDangling")
    else {
        return Err("actual original dangling GUID".into());
    };
    assert!(
        forge.record_by_guid(*guid).is_none(),
        "the actual original Forge contains no target for this stored GUID"
    );
    assert_eq!(
        guid.to_string(),
        "00000000-0000-0000-0000-000000006813",
        "the actual dangling token preserves its authored bits"
    );
    assert!(
        matches!(
            tree.field("lateBad"),
            Some(Value::Error(ValueError::InvalidBool { byte: 2 }))
        ),
        "the later original bool is genuinely malformed"
    );
    assert!(
        matches!(tree.field("lateNumber"), Some(Value::Single(value)) if value.to_bits() == NONFINITE_BITS),
        "the later original Single keeps its exact IEEE payload bits"
    );
    Ok(())
}
