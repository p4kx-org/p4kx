// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_extraction::{DataForge, SourceValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::tag_catalog::{self, TagPathLookup, TagPathMeaning};

use super::super::TagInputs;

pub(super) const STEM: usize = 24;
pub(super) const TAGS: usize = STEM + 2;
pub(super) const CALLERS: usize = 32;
const FIRST_TAG: u128 = 0xb100_0000_0000_4000_8000_0000_0000_0000;

pub(super) fn bytes() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("ContractTemplate")
                .array("resourceTags", TestType::ReferenceTo("Tag".to_owned())),
        );
    for position in 0..TAGS {
        builder = builder.record(tag_record(position));
    }
    builder
        .record(
            TestRecord::new(
                FIRST_TAG + 100,
                "TagDatabase",
                "TagDatabase.Root",
                "tags/root.xml",
            )
            .value(
                "tags",
                TestValue::Array(vec![TestValue::Reference(guid(0))]),
            ),
        )
        .record(
            TestRecord::new(
                FIRST_TAG + 101,
                "ContractTemplate",
                "ContractTemplate.Callers",
                "templates/callers.xml",
            )
            .value(
                "resourceTags",
                TestValue::Array(
                    (0..CALLERS)
                        .map(|index| TestValue::Reference(guid(STEM + index % 2)))
                        .collect(),
                ),
            ),
        )
        .build()
}

fn tag_record(position: usize) -> TestRecord {
    let children = match (position + 1).cmp(&STEM) {
        Ordering::Less => vec![TestValue::Reference(guid(position + 1))],
        Ordering::Equal => {
            vec![
                TestValue::Reference(guid(STEM)),
                TestValue::Reference(guid(STEM + 1)),
            ]
        }
        Ordering::Greater => Vec::new(),
    };
    TestRecord::new(
        guid(position),
        "Tag",
        format!("Tag.Node{position}"),
        format!("tags/node{position}.xml"),
    )
    .value("tagName", TestValue::text("tag"))
    .value("children", TestValue::Array(children))
}

pub(super) fn selected<'input>(
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
) -> TestResult<TagInputs> {
    let captured = tag_catalog::read(forge, input, None, false)?;
    assert_eq!(
        captured.context.tree().len(),
        TAGS,
        "every physical selected Tag survives"
    );
    let record = forge
        .record(u32::try_from(TAGS + 1)?)
        .ok_or("actual Template caller")?;
    let tree = record.tree()?;
    assert_eq!(tree.error_count(), 0, "all original operands are readable");
    let Some(Value::Array(values)) = tree.field("resourceTags") else {
        return Err("actual authored Tag array".into());
    };
    assert_eq!(
        values.len(),
        CALLERS,
        "all repeated physical slots are present"
    );
    let scope = input.record_scope(&record, &tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let mut inputs = TagInputs::local(Vec::new(), true);
    for (index, value) in values.iter().enumerate() {
        let Value::Reference(reference) = value else {
            return Err("actual reference operand".into());
        };
        let position = STEM + index % 2;
        assert_eq!(
            reference.record_index(),
            u32::try_from(position)?,
            "the exact physical target"
        );
        let target = forge
            .record(u32::try_from(position)?)
            .ok_or("actual selected target")?;
        assert_eq!(
            reference.guid(),
            target.guid(),
            "the exact original target GUID"
        );
        let observed = fields.fact(value)?.reference_observation()?;
        let TagPathLookup::Ready {
            meaning: TagPathMeaning::Path,
            value,
        } = observed.tag_path(&captured.context)?
        else {
            return Err("actual selected Tag path".into());
        };
        let (value, local) = value.split();
        assert!(
            matches!(value, SourceValue::Present(_)),
            "each original operand resolves"
        );
        inputs.merge(local)?;
    }
    Ok(inputs)
}

const fn guid(position: usize) -> u128 {
    FIRST_TAG + position as u128
}
