// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, Guid};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::CatalogReadFailure;
use crate::picked::Picked;

#[test]
fn a_ship_whose_base_data_cannot_be_read_becomes_a_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Owner")
                .field("value", TestType::UInt32)
                .declared_size(5),
        )
        .record(
            TestRecord::new(
                u128::from_le_bytes([1; 16]),
                "Owner",
                "Owner.One",
                "libs/foundry/records/entities/spaceships/x.xml",
            )
            .value("value", TestValue::UInt32(7)),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual original failed Record")?;
    assert_eq!(
        record.guid(),
        Guid::from_stored([1; 16]),
        "the original GUID is unchanged"
    );
    let error = record
        .tree()
        .expect_err("the original canonical read really fails");
    assert!(
        matches!(
            error,
            DataForgeError::StructLayout {
                declared: 5,
                fields: 4,
                ..
            }
        ),
        "the fixture preserves a real source failure before the mapping assertion",
    );
    let failure = CatalogReadFailure::of(&record, &error);
    assert_eq!(
        failure.clone().into_interpreted().record_path,
        "libs/foundry/records/entities/spaceships/x",
        "original path"
    );
    let picked = Picked::of_ship(Err(failure.clone()), |_| std::process::abort());
    assert!(
        matches!(picked, Picked::Failed(found) if found == failure),
        "a failed ship read keeps the same failure values",
    );
    Ok(())
}
