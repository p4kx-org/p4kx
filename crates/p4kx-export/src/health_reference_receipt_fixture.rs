// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::{OriginalValueEvent, ReferenceCapture, ReferenceObservation};
use crate::record_observation::RecordObservation;

pub(super) fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner").field(
            "link",
            TestType::ReferenceTo("SCItemManufacturer".to_owned()),
        ))
        .structure(TestStruct::new("SCItemManufacturer"))
        .record(
            TestRecord::new(0x6811, "Owner", "Owner.One", "owner.xml")
                .value("link", TestValue::Reference(0x6812)),
        )
        .record(TestRecord::new(
            0x6812,
            "SCItemManufacturer",
            "SCItemManufacturer.Target",
            "manufacturers/target.xml",
        ))
        .build()
}

pub(super) fn observation(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult<ReferenceObservation> {
    let owner = forge.record(0).ok_or("actual owner Record")?;
    let target = forge.record(1).ok_or("actual target Record")?;
    let tree = owner.tree()?;
    let admission = RecordObservation::capture(input, &owner)?;
    let scope = input.record_scope(&owner, &tree)?;
    let capture = ReferenceCapture::new(input, &admission, &scope)?;
    let mut walk = scope.walk(None);
    let Some(OriginalValueEvent::Value(site)) = walk.next_event()? else {
        return Err("actual original Reference visit".into());
    };
    let Value::Reference(reference) = site.value() else {
        return Err("actual original Reference value".into());
    };
    assert_eq!(site.field().name(), "link", "the original declaration");
    let storage = site.field().storage();
    assert_eq!(
        storage.data_type(),
        DataType::Reference,
        "the original field stores a Reference GUID"
    );
    assert!(!storage.is_array(), "the original single Reference");
    assert_eq!(storage.target(), 1, "the genuine target declaration index");
    assert_eq!(
        reference.record_index(),
        target.index(),
        "the actual target position"
    );
    assert_eq!(reference.guid(), target.guid(), "the actual target GUID");
    assert!(
        reference.original_target().shares_source_with(&owner),
        "the original target belongs to this Forge"
    );
    Ok(capture.reference(&site)?)
}
