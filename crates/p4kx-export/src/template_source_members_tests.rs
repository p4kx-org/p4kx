// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceEvidenceError;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::TemplateMembers;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::template_source_refs::SourceFieldRef;
use crate::{CatalogError, ExportAccountingError};

#[test]
fn selected_spans_keep_exact_dfs_order_and_reject_gaps_and_foreign_facts() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Template")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let [outer, child, gap, tail] = fields.facts() else {
            return Err("actual root, child, excluded root and last root".into());
        };
        let members = TemplateMembers::new(
            &fields,
            [tail, child, outer, outer],
            FlowOwnerKind::ContractTemplate,
        )?;
        assert_order(&members);
        assert_refs(&members, child, tail)?;
        assert!(
            matches!(
                members.value_ref(gap),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "a gap is not assigned its predecessor's arena index"
        );
        let other_scope = input.record_scope(&record, &tree)?;
        let other = OriginalFlowFields::read(&other_scope, None)?;
        let original = other
            .facts()
            .first()
            .ok_or("same ordinal in another frame")?;
        assert!(
            matches!(
                members.value_ref(original),
                Err(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "identical original ordinals do not grant cross-frame membership"
        );
        Ok(())
    })
}

fn assert_order(members: &TemplateMembers<'_, '_, '_, '_>) {
    assert_eq!(
        members.spans.len(),
        2,
        "two ordered disjoint admitted spans"
    );
    assert_eq!(members.value_count(), 3, "overlap and repeats emit once");
    assert_eq!(
        members
            .roots()
            .map(OriginalValueFact::ordinal)
            .collect::<Vec<_>>(),
        [0, 3],
        "the contained child never becomes a separate root"
    );
    assert_eq!(
        members
            .values()
            .map(OriginalValueFact::ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 3],
        "original occurrence order survives skipped and reordered root input"
    );
}

fn assert_refs<'scope, 'read, 'source>(
    members: &TemplateMembers<'_, 'scope, 'read, 'source>,
    child: &OriginalValueFact<'scope, 'read, 'source>,
    tail: &OriginalValueFact<'scope, 'read, 'source>,
) -> TestResult {
    assert!(
        matches!(
            members.field_ref(child)?,
            SourceFieldRef::Field { parent, field_index }
                if parent.value_index.get() == 0 && field_index.get() == 0
        ),
        "a nested field retains its source parent and field ordinal"
    );
    assert!(
        matches!(members.field_ref(tail)?, SourceFieldRef::Root { root_field_index }
            if root_field_index.get() == 1),
        "the second root has its own root index"
    );
    assert_eq!(
        members.value_ref(tail)?.value_index.get(),
        2,
        "the second root's dense value index excludes the gap"
    );
    Ok(())
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ContractTemplate")
                .field("outer", TestType::Class("Context".to_owned()))
                .field("gap", TestType::UInt32)
                .field("tail", TestType::UInt32),
        )
        .structure(TestStruct::new("Context").field("child", TestType::UInt32))
        .record(
            TestRecord::new(
                0xf1b0_0000_0000_4000_8000_0000_0000_0002,
                "ContractTemplate",
                "ContractTemplate.Members",
                "libs/foundry/records/contracts/contracttemplates/members.xml",
            )
            .value(
                "outer",
                TestValue::Struct(
                    TestInstance::new("Context").value("child", TestValue::UInt32(7)),
                ),
            )
            .value("gap", TestValue::UInt32(8))
            .value("tail", TestValue::UInt32(9)),
        )
        .build()
}
